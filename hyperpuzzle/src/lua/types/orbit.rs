use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use hypermath::pga::Motor;
use hypermath::ApproxHashMap;
use itertools::Itertools;

use crate::lua::lua_warn_fn;
use crate::util::lazy_resolve;

use super::*;

/// Lua orbit object.
#[derive(Debug, Clone)]
pub struct LuaOrbit {
    symmetry: LuaSymmetry,
    init: Vec<Transformable>,

    /// Whether names have been assigned.
    has_names: bool,
    /// Indices into `orbit_list`, in iteration order. If `None`, it is assumed
    /// to be equivalent to `0..orbit_len.len()`.
    order: Option<Vec<usize>>,
    /// Elements, in the order that they were generated.
    orbit_list: Vec<OrbitElement>,

    iter_index: Arc<AtomicUsize>,
}

impl<'lua> FromLua<'lua> for LuaOrbit {
    fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        cast_userdata(lua, &value)
    }
}

impl LuaUserData for LuaOrbit {
    fn add_fields<'lua, F: LuaUserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_meta_field("type", LuaStaticStr("orbit"));

        fields.add_field_method_get("symmetry", |_lua, this| Ok(this.symmetry.clone()));
        fields.add_field_method_get("init", |lua, this| {
            lua.create_sequence_from(
                this.init
                    .iter()
                    .map(|t| t.into_nillable_lua(lua))
                    .collect::<LuaResult<Vec<_>>>()?,
            )
        });

        fields.add_field_method_get("names", |lua, this| {
            this.has_names
                .then(|| {
                    lua.create_sequence_from(this.orbit_list.iter().map(|elem| elem.name.clone()))
                })
                .transpose()
        });
        fields.add_field_method_get("displays", |lua, this| {
            this.has_names
                .then(|| {
                    lua.create_sequence_from(
                        this.orbit_list.iter().map(|elem| elem.display.clone()),
                    )
                })
                .transpose()
        });
    }

    fn add_methods<'lua, M: LuaUserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(LuaMetaMethod::Len, |_lua, this, ()| {
            Ok(this.orbit_list.len())
        });

        methods.add_meta_method(LuaMetaMethod::Call, |lua, this, ()| {
            // Get the index of the Lua iteration.
            let iter_index = this.iter_index.fetch_add(1, Ordering::Relaxed);
            // Look up that index in the custom ordering, if there is one.
            let orbit_index = match &this.order {
                Some(order) => order.get(iter_index).copied(),
                None => Some(iter_index),
            };

            // Return multiple values.
            let mut values = vec![];
            if let Some(i) = orbit_index {
                if let Some(element) = this.orbit_list.get(i) {
                    let OrbitElement {
                        transform,
                        name,
                        display,
                        objects,
                    } = element;
                    // The first value is the transform.
                    values.push(LuaTransform(transform.clone()).into_lua(lua)?);
                    // Then push the objects.
                    for obj in objects {
                        values.push(obj.into_nillable_lua(lua)?);
                    }
                    // If custom names are given, then the last values are the
                    // custom names.
                    if this.has_names {
                        values.push(name.as_deref().into_lua(lua)?);
                        values.push(display.as_deref().into_lua(lua)?);
                    }
                }
            }
            Ok(LuaMultiValue::from_vec(values))
        });

        methods.add_method("iter", |_lua, this, ()| {
            Ok(Self {
                iter_index: Arc::new(AtomicUsize::new(0)),
                ..this.clone()
            })
        });

        methods.add_method("with", |lua, this, arg| {
            let Some(names_table) = arg else {
                lua.warning("orbit:with() called nil value", false);
                return Ok(this.clone());
            };

            let mut motor_to_name = ApproxHashMap::new();
            for (name, mirror_seq) in names_from_table(lua, names_table)? {
                let motor = this.symmetry.motor_for_mirror_seq(mirror_seq)?;
                if let Some(existing_name) = motor_to_name.insert(motor, name.clone()) {
                    lua.warning(
                        format!("duplicate mirror sequence: {name:?} and {existing_name:?}"),
                        false,
                    );
                }
            }

            let mut ret = this.clone();
            ret.has_names = true;
            for elem in &mut ret.orbit_list {
                if let Some(name) = motor_to_name.get(&elem.transform) {
                    elem.name = Some(name.clone());
                }
            }
            Ok(ret)
        });

        methods.add_method("with_names_and_order", |lua, this, arg| {
            let Some(names_and_order_table) = arg else {
                lua.warning("orbit:with_names_and_order() called with nil value", false);
                return Ok(this.clone());
            };

            if this.order.is_some() {
                return Err(LuaError::external("orbit already has names and ordering"));
            }
            let names_and_order = names_and_order_from_table(lua, names_and_order_table)?;
            let mut lookup = ApproxHashMap::new();
            for (i, element) in this.orbit_list.iter().enumerate() {
                lookup.insert(element.objects.clone(), i);
            }
            let mut order = vec![];
            let mut new_orbit_list = this.orbit_list.clone();
            let mut seen: Vec<bool> = vec![false; new_orbit_list.len()];
            for ((name, display), motor) in names_and_order {
                if let Some(&index) = lookup.get(&motor.transform(&this.init)) {
                    seen[index] = true;
                    let element = &mut new_orbit_list[index];
                    if let Some(old_name) = &element.name {
                        let msg =
                            format!("duplicate in symmetry orbit order: {old_name:?} and {name:?}");
                        lua.warning(msg, false);
                    } else {
                        element.name = Some(name);
                        order.push(index);
                    }
                    element.display = display;
                }
            }

            // Check for missing elements.
            for i in seen.iter().positions(|&b| !b) {
                order.push(i);
            }

            Ok(Self {
                symmetry: this.symmetry.clone(),
                init: this.init.clone(),

                has_names: true,
                order: Some(order),
                orbit_list: new_orbit_list,

                iter_index: Arc::new(AtomicUsize::new(0)),
            })
        });
    }
}

impl LuaOrbit {
    /// Returns the orbit of `init` under `symmetry`.
    pub fn new(symmetry: LuaSymmetry, init: Vec<Transformable>) -> Self {
        let orbit_list = symmetry
            .orbit(init.clone())
            .into_iter()
            // Assign empty names.
            .map(|(transform, objects)| OrbitElement {
                transform,
                name: None,
                display: None,
                objects,
            })
            .collect();

        Self {
            symmetry,
            init,

            has_names: false,
            order: None,
            orbit_list,

            iter_index: Arc::new(AtomicUsize::new(0)),
        }
    }
    /// Returns the symmetry used to generate the orbit.
    pub fn symmetry(&self) -> &LuaSymmetry {
        &self.symmetry
    }
    /// Returns the initial seed objects that this is the orbit of.
    pub fn init(&self) -> &[Transformable] {
        &self.init
    }
    /// Returns whether the orbit has custom names assigned to any elements.
    pub fn has_names(&self) -> bool {
        // Ok technically it's possible to have `names = Some(vec![])` in which
        // case no elements have any names. But the comment above is accurate
        // enough.
        self.has_names
    }
    /// Returns an iterator over the whole orbit.
    fn iter_in_order(&self) -> impl Iterator<Item = &OrbitElement> {
        match &self.order {
            Some(order) => order.iter().flat_map(|&i| self.orbit_list.get(i)).collect(),
            None => self.orbit_list.iter().collect_vec(),
        }
        .into_iter()
    }
}

/// Constructs an assignment of names based on a table for a particular symmetry
/// group.
///
/// The keys are mirror sequences and the values are names.
pub fn names_from_table<'lua>(
    lua: &'lua Lua,
    table: LuaTable<'lua>,
) -> LuaResult<Vec<(String, Vec<usize>)>> {
    let mut key_value_dependencies = vec![];

    for pair in table.pairs() {
        let (k, v) = pair?;
        let (mirror_seq, init_name) = mirror_seq_and_opt_name_from_value(lua, v)?;
        key_value_dependencies.push((k, (mirror_seq, init_name)));
    }

    // Resolve lazy evaluation.
    Ok(lazy_resolve(
        key_value_dependencies,
        |mut seq1, seq2| {
            // TODO: O(n^2)
            seq1.extend(seq2);
            seq1
        },
        lua_warn_fn(lua),
    )
    .into_iter()
    .collect())
}

/// Constructs an assignment of names and ordering based on a table for a
/// particular symmetry group.
///
/// The first string in each pair is the name, which is required. The second
/// string in each pair is the display name, which is optional.
pub fn names_and_order_from_table<'lua>(
    lua: &'lua Lua,
    table: LuaTable<'lua>,
) -> LuaResult<Vec<((String, Option<String>), Motor)>> {
    // TODO: just compare against the existing symmetry, and use the existing
    // symmetry for calculations
    let symmetry = LuaSymmetry::construct_from_lua_value(table.get("symmetry")?)?;

    let mut order = vec![];

    let mut key_value_dependencies = vec![];

    for entry in table.sequence_values::<LuaValue<'_>>() {
        let [key, name, display]: [LuaValue<'_>; 3] = <_>::from_lua(entry?, lua)?;
        let name = String::from_lua(name, lua)?;
        let display = Option::<String>::from_lua(display, lua)?;
        order.push((name.clone(), display));

        let (mirror_seq, init_name) = mirror_seq_and_opt_name_from_value(lua, key)?;
        let motor = symmetry.motor_for_mirror_seq(mirror_seq)?;

        key_value_dependencies.push((name, (motor, init_name)));
    }

    // Resolve lazy evaluation.
    let mut map = lazy_resolve(key_value_dependencies, |m1, m2| m1 * m2, lua_warn_fn(lua));

    // Assemble into ordered list.
    Ok(order
        .into_iter()
        .filter_map(|(name, display)| {
            let motor = map.remove(&name)?;
            Some(((name, display), motor))
        })
        .collect())
}

/// Symmetric set of a particular type of object.
#[derive(Debug, Clone)]
pub enum LuaSymmetricSet<T> {
    /// Single object (using the trivial symmetry).
    Single(T),
    /// Symmetric orbit of an object.
    Orbit(LuaOrbit),
}
impl<'lua, T: LuaTypeName + FromLua<'lua>> FromLua<'lua> for LuaSymmetricSet<T> {
    fn from_lua(value: LuaValue<'lua>, lua: &'lua Lua) -> LuaResult<Self> {
        if let Ok(orbit) = <_>::from_lua(value.clone(), lua) {
            Ok(Self::Orbit(orbit))
        } else if let Ok(v) = <_>::from_lua(value.clone(), lua) {
            Ok(Self::Single(v))
        } else {
            // This error isn't quite accurate, but it's close enough. The error
            // message will say that we need a value of type `T`, but in fact we
            // accept an orbit of `T` as well.
            lua_convert_err(&value, T::type_name(lua)?)
        }
    }
}
impl<'lua, T: LuaTypeName + FromLua<'lua> + Clone> LuaSymmetricSet<T> {
    /// Returns a list of all the objects in the orbit.
    pub fn to_vec(&self, lua: &'lua Lua) -> LuaResult<Vec<((Option<String>, Option<String>), T)>> {
        match self {
            LuaSymmetricSet::Single(v) => Ok(vec![((None, None), v.clone())]),
            LuaSymmetricSet::Orbit(orbit) => orbit
                .iter_in_order()
                .map(|element| {
                    let v = Self::to_expected_type(lua, element.objects.get(0))?;
                    Ok(((element.name.clone(), element.display.clone()), v))
                })
                .try_collect(),
        }
    }
    /// Returns the initial object from which the others are generated.
    pub fn first(&self, lua: &'lua Lua) -> LuaResult<T> {
        match self {
            LuaSymmetricSet::Single(v) => Ok(v.clone()),
            LuaSymmetricSet::Orbit(orbit) => Self::to_expected_type(lua, orbit.init().get(0)),
        }
    }

    fn to_expected_type(lua: &'lua Lua, maybe_obj: Option<&Transformable>) -> LuaResult<T> {
        let lua_value =
            maybe_obj
                .and_then(|obj| obj.into_lua(lua))
                .ok_or(LuaError::external(format!(
                    "expected orbit of {}",
                    T::type_name(lua)?,
                )))??;
        T::from_lua(lua_value, lua)
    }
}

/// Element in an orbit.
#[derive(Debug, Clone)]
struct OrbitElement {
    transform: Motor,
    name: Option<String>,
    display: Option<String>,
    objects: Vec<Transformable>,
}

fn mirror_seq_and_opt_name_from_value<'lua>(
    lua: &'lua Lua,
    value: LuaValue<'lua>,
) -> LuaResult<(Vec<usize>, Option<String>)> {
    let mut seq: Vec<LuaValue<'_>> = LuaTable::from_lua(value, lua)?
        .sequence_values::<LuaValue<'_>>()
        .try_collect()?;
    let init_name = match seq.last().cloned() {
        Some(LuaValue::String(s)) => {
            seq.pop();
            Some(s.to_string_lossy().to_string())
        }
        _ => None,
    };
    let mirror_indices: Vec<usize> = seq
        .into_iter()
        .map(|v| LuaIndex::from_lua(v, lua).map(|LuaIndex(i)| i))
        .try_collect()?;
    Ok((mirror_indices, init_name))
}