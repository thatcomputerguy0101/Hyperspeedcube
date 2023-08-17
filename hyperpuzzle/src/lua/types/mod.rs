use rlua::prelude::*;

#[macro_use]
mod wrappers;
mod manifold;
mod multivector;
mod ndim;
mod shapeset;
mod space;
mod vector;

pub use manifold::LuaManifold;
pub use multivector::{LuaConstructMultivector, LuaMultivector};
pub use ndim::LuaNdim;
pub use shapeset::LuaShapeSet;
pub use space::LuaSpace;
pub use vector::{LuaConstructVector, LuaVector};
pub use wrappers::*;

pub fn lua_type_name(lua_value: &LuaValue<'_>) -> &'static str {
    if let LuaValue::UserData(userdata) = lua_value {
        macro_rules! return_name_if_type {
            ($userdata:ident, $wrapper_type:ty) => {
                if $userdata
                    .is::<LuaNamedUserData<<$wrapper_type as LuaUserDataConvertWrap>::Inner>>()
                {
                    return <$wrapper_type as LuaUserDataConvertWrap>::TYPE_NAME;
                }
            };
        }
        return_name_if_type!(userdata, LuaManifold);
        return_name_if_type!(userdata, LuaMultivector);
        return_name_if_type!(userdata, LuaShapeSet);
        return_name_if_type!(userdata, LuaSpace);
        return_name_if_type!(userdata, LuaVector);
    }
    lua_value.type_name()
}

#[derive(Debug, Clone)]
pub struct LuaLogLine {
    pub msg: String,
    pub file: String,
    pub level: String,
}
impl<'lua> TryFrom<LuaTable<'lua>> for LuaLogLine {
    type Error = LuaError;

    fn try_from(value: LuaTable<'lua>) -> std::result::Result<Self, Self::Error> {
        Ok(LuaLogLine {
            msg: value.get("msg")?,
            file: value.get("file")?,
            level: value.get("level")?,
        })
    }
}

#[derive(Debug, Clone)]
pub enum LuaFileLoadError {
    MissingDependencies(Vec<String>),
    UserError(LuaError),
    InternalError(LuaError),
}