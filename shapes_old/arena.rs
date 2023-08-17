//! Algorithm for slicing space into pieces based on Andrey Astrelin's
//! implementation of `GenCube()` in Magic Puzzle Ultimate (FaceCuts.cs).

use itertools::Itertools;
use slab::Slab;
use std::fmt;
use std::ops::{Index, Neg};
use tinyset::Set64;

use super::log::ShapeConstructionLog;
use super::{manifold::*, shape::*};
use hypermath::prelude::*;

/// Set of shapes in a space.
///
/// A shape arena is always initialized with a single shape representing the
/// whole space.
#[derive(Debug, Clone)]
pub struct ShapeArena {
    /// Space that all shapes inhabit.
    space: Manifold,
    /// All shapes and elements of them.
    shapes: Slab<Shape>,
    /// Top-level "root" shapes.
    roots: Vec<ShapeId>,

    /// Shape construction log (for debugging).
    log: ShapeConstructionLog,
}

impl Index<ShapeId> for ShapeArena {
    type Output = Shape;

    fn index(&self, index: ShapeId) -> &Self::Output {
        &self.shapes[index.0 as usize]
    }
}

impl fmt::Display for ShapeArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Shape arena in space {}", self.space)?;
        for &root_id in &self.roots {
            self.display_shape(f, ShapeRef::from(root_id), Sign::Pos, 1)?;
        }
        Ok(())
    }
}

impl ShapeArena {
    /// Constructs a new shape arena containing only a shape representing the
    /// whole space.
    pub fn new(space: Manifold) -> Self {
        let mut shapes = Slab::new();
        let id = shapes.insert(Shape::whole_space(space.clone()));
        let roots = vec![ShapeId(id as u32)];

        Self {
            space,
            shapes,
            roots,

            log: ShapeConstructionLog::default(),
        }
    }

    /// Dumps a log file to the disk.
    pub fn dump_log_file(&self) {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();
        match std::fs::write(
            format!("shape_construction_log_{time}.log"),
            self.log.to_string(),
        ) {
            Ok(()) => (),
            Err(e) => log::error!("failed to write log file: {e}"),
        }
    }

    /// Returns the manifold representing the whole space.
    pub fn space(&self) -> &Manifold {
        &self.space
    }
    /// Returns the list of root shapes, in canonical order based on the cuts.
    pub fn roots(&self) -> &[ShapeId] {
        &self.roots
    }
    /// Returns whether the shape arena is empty (contains no root shapes).
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Adds a shape.
    fn add(&mut self, shape: Shape) -> Result<ShapeRef> {
        let ev = self.log.event("add", "Adding shape");
        ev.log_value("manifold", &shape.manifold);
        ev.log_set64("boundary", &shape.boundary);

        // Check the rank of each boundary shape.
        let ndim = shape.manifold.ndim()?;
        for boundary_shape in shape.boundary.iter() {
            ensure!(ndim == self[boundary_shape.id].ndim()? + 1);
        }

        // Check that polygons are topologically valid.
        if cfg!(debug_assertions) && ndim == 2 {
            let mut starting_points = vec![];
            let mut ending_points = vec![];
            for edge in shape.boundary.iter() {
                for point_pair in self[edge.id].boundary.iter() {
                    let [a, b] = self.shape_to_point_pair(point_pair * edge.sign)?;

                    match ending_points.iter().find_position(|p| approx_eq(&a, p)) {
                        Some((i, _)) => {
                            ending_points.remove(i);
                        }
                        None => starting_points.push(a),
                    };
                    match starting_points.iter().find_position(|p| approx_eq(&b, p)) {
                        Some((i, _)) => {
                            starting_points.remove(i);
                        }
                        None => ending_points.push(b),
                    };
                }
            }
            if !starting_points.is_empty() || !ending_points.is_empty() {
                self.log.event("error", "Error! Invalid polygon");
                bail!("error! invalid polygon");
            }
        }

        let idx = self.shapes.insert(shape);
        ev.log_value("id", idx);

        Ok(ShapeRef {
            id: ShapeId(idx as _),
            sign: Sign::Pos,
        })
    }
    /// Adds a shape using the manifold of an existing shape, or reuses the
    /// existing shape if possible. In particular, if `new_boundary` is the same
    /// as the boundary of `old_shape`, returns `old_shape`.
    fn add_subshape(
        &mut self,
        old_shape: ShapeId,
        new_boundary: Set64<ShapeRef>,
    ) -> Result<ShapeRef> {
        let ev = self.log.event("add_subshape", "Adding subshape");
        ev.log_value("old_shape", old_shape);
        ev.log_set64("new_boundary", &new_boundary);

        if new_boundary == self[old_shape].boundary {
            ev.log("same as existing shape");

            Ok(ShapeRef::from(old_shape))
        } else {
            ev.log("creating new shape");

            let new_shape = self.add(Shape::new(self[old_shape].manifold.clone(), new_boundary))?;

            // Copy metadata from old shape.
            self.set_metadata(new_shape, self.get_metadata(ShapeRef::from(old_shape)));
            self.set_metadata(-new_shape, self.get_metadata(-ShapeRef::from(old_shape)));

            Ok(new_shape)
        }
    }

    /// Returns a shape's manifold, flipped depending on the sign of the
    /// reference to the shape.
    pub fn signed_manifold_of_shape(&self, shape: ShapeRef) -> Result<Manifold> {
        let m = &self[shape.id].manifold;
        match shape.sign {
            Sign::Pos => Ok(m.clone()),
            Sign::Neg => m.flip(),
        }
    }
    /// Returns the sign difference between two manifolds, or `None` if they are
    /// different.
    fn sign_difference(
        &self,
        a: impl SignedManifold,
        b: impl SignedManifold,
    ) -> Result<Option<Sign>> {
        match a
            .get_manifold_from(self)?
            .relative_orientation(b.get_manifold_from(self)?)
        {
            Some(sign) => Ok(Some(sign * a.sign() * b.sign())),
            None => Ok(None),
        }
    }

    pub fn get_metadata(&self, shape: ShapeRef) -> Option<ShapeMetadata> {
        match shape.sign {
            Sign::Pos => self[shape.id].positive_metadata,
            Sign::Neg => self[shape.id].negative_metadata,
        }
    }
    fn set_metadata(&mut self, shape: ShapeRef, metadata: Option<ShapeMetadata>) {
        let shape_mut = &mut self.shapes[shape.id.0 as usize];
        match shape.sign {
            Sign::Pos => shape_mut.positive_metadata = metadata,
            Sign::Neg => shape_mut.negative_metadata = metadata,
        }
    }

    /// Garbage-collects unused shapes.
    pub fn gc(&mut self) {
        let ev = self.log.event("gc", "Running garbage collection");

        let total = self.shapes.len();
        let mut keys_to_delete = self
            .shapes
            .iter()
            .map(|(i, _shape)| ShapeId(i as u32))
            .collect();
        for &root in &self.roots {
            self.gc_mark_recursive(&mut keys_to_delete, root);
        }
        let num_deleted = keys_to_delete.len();
        for id in keys_to_delete.iter() {
            ev.log(format!("Deleting {id}"));
            self.shapes.remove(id.0 as usize);
        }
        let percent = num_deleted * 100 / total;
        ev.log(format!(
            "Garbage-collected {num_deleted}/{total} shapes ({percent}%)",
        ));
    }
    fn gc_mark_recursive(&self, keys_to_delete: &mut Set64<ShapeId>, shape_to_keep: ShapeId) {
        if keys_to_delete.remove(&shape_to_keep) {
            for child in self[shape_to_keep].boundary.iter() {
                self.gc_mark_recursive(keys_to_delete, child.id);
            }
        }
    }

    /// Cuts all shapes in the arena.
    pub fn cut(&mut self, params: CutParams) -> Result<()> {
        let ev = self
            .log
            .event("cut_all", format!("Cutting all shapes by {}", params.cut));
        ev.log_value("inside", params.inside);
        ev.log_value("outside", params.outside);

        let mut op = SliceOperation::new(params.clone());

        for root_id in std::mem::take(&mut self.roots) {
            match self
                .cut_shape(ShapeRef::from(root_id), &mut op)
                .context("error cutting shape")?
            {
                ShapeSplit::Flush | ShapeSplit::ManifoldInside | ShapeSplit::ManifoldOutside => {
                    bail!("root shape is manifold is not split by cut")
                }
                ShapeSplit::NonFlush {
                    inside,
                    outside,
                    intersection_shape: _,
                } => {
                    if let Some(s) = inside {
                        match params.inside {
                            CutOp::Remove => ev.log(format!("Ignoring inside shape {s}")),
                            CutOp::Keep(_) => {
                                ev.log(format!("Adding inside shape {s} as root"));
                                self.roots.push(s.id);
                            }
                        }
                    }

                    if let Some(s) = outside {
                        match params.outside {
                            CutOp::Remove => ev.log(format!("Ignoring outside shape {s}")),
                            CutOp::Keep(_) => {
                                ev.log(format!("Adding outside shape {s} as root"));
                                self.roots.push(s.id);
                            }
                        }
                    }
                }
            }
        }

        // self.gc();

        Ok(())
    }

    /// Cuts a shape.
    fn cut_shape(&mut self, shape: ShapeRef, slice_op: &mut SliceOperation) -> Result<ShapeSplit> {
        let ev = self
            .log
            .event("cut_shape", format!("Cutting shape {shape}"));
        let result = match slice_op.results_cache.get(&shape.id) {
            Some(result) => {
                ev.log("Using cached split result");

                result.clone()
            }
            None => {
                let result = self
                    .cut_shape_uncached(shape.id, slice_op)
                    .with_context(|| format!("error cutting shape {shape}"))?;

                slice_op.results_cache.insert(shape.id, result.clone());
                result
            }
        };
        ev.log_value("result", &result);

        // Add metadata.
        if let ShapeSplit::NonFlush {
            inside,
            outside,

            intersection_shape,
        } = &result
        {
            // TODO: clean this up
            if let Some(s) = inside {
                self.shapes[s.id.0 as usize].positive_metadata = self[shape.id].positive_metadata;
                self.shapes[s.id.0 as usize].negative_metadata = self[shape.id].negative_metadata;
            }
            if let Some(s) = outside {
                self.shapes[s.id.0 as usize].positive_metadata = self[shape.id].positive_metadata;
                self.shapes[s.id.0 as usize].negative_metadata = self[shape.id].negative_metadata;
            }
            if let Some(s) = intersection_shape {
                self.set_metadata(*s, slice_op.inside_metadata);
                self.set_metadata(-*s, slice_op.outside_metadata);
            }
        }

        Ok(match shape.sign {
            Sign::Pos => result,
            Sign::Neg => -result,
        })
    }
    /// Cuts a shape without caching the result.
    fn cut_shape_uncached(
        &mut self,
        shape: ShapeId,
        slice_op: &mut SliceOperation,
    ) -> Result<ShapeSplit> {
        let ev = self.log.event("cut", format!("Cutting shape {shape}"));

        match self[shape]
            .manifold
            .split(&slice_op.divider, &self.space)
            .context("error splitting manifold")?
        {
            ManifoldSplit::Flush => {
                ev.log("Manifold is flush with cut");
                Ok(ShapeSplit::Flush)
            }
            ManifoldSplit::Inside => {
                ev.log("Manifold is entirely inside");
                Ok(ShapeSplit::ManifoldInside)
            }
            ManifoldSplit::Outside => {
                ev.log("Manifold is entirely outside");
                Ok(ShapeSplit::ManifoldOutside)
            }

            ManifoldSplit::Split {
                intersection_manifold,
            } if self[shape].ndim()? == 1 => {
                ev.log("Manifold is split (1D)");

                ev.log("Adding intersection shape point pair");
                let intersection_shape = self.add(Shape::whole_space(intersection_manifold))?;
                ev.log_value("intersection_shape", intersection_shape);

                ev.log("Simplifying inside boundary");
                let inside_boundary = self
                    .incremental_simplify_intervals_intersection(
                        &self[shape].boundary.clone(),
                        intersection_shape,
                        &self[shape].manifold.clone(),
                    )
                    .context("error simplifying 1D boundary of inside")?;
                let inside = if let Some(boundary) = inside_boundary {
                    ev.log("Adding inside shape");
                    Some(self.add_subshape(shape, boundary)?)
                } else {
                    ev.log("No inside shape");
                    None
                };

                ev.log("Simplifying outside boundary");
                let outside_boundary = self
                    .incremental_simplify_intervals_intersection(
                        &self[shape].boundary.clone(),
                        -intersection_shape,
                        &self[shape].manifold.clone(),
                    )
                    .context("error simplifying 1D boundary of outside")?;
                let outside = if let Some(boundary) = outside_boundary {
                    ev.log("Adding outside shape");
                    Some(self.add_subshape(shape, boundary)?)
                } else {
                    ev.log("No outside shape");
                    None
                };

                Ok(ShapeSplit::NonFlush {
                    inside,
                    outside,
                    intersection_shape: Some(intersection_shape),
                })
            }

            ManifoldSplit::Split {
                intersection_manifold,
            } => {
                ev.log("Manifold is split (2D+)");

                // (N-1)-dimensional shapes that together comprise the boundary
                // of `shape ∩ cut`
                let mut self_boundary_of_inside = Set64::new();
                // (N-1)-dimensional shapes that together comprise the boundary
                // of `shape ∩ ~cut`
                let mut self_boundary_of_outside = Set64::new();
                // (N-2)-dimensional shapes that together comprise the boundary
                // of `shape ∩ boundary(cut)`
                let mut intersection_boundary = Set64::new();
                // (N-1)-dimensional shape that is `shape ∩ boundary(cut)`
                let mut intersection_shape = None;

                // Split each of the "child" shapes that comprise the boundary
                // of `shape`.
                for child in self[shape].boundary.clone() {
                    match self.cut_shape(child, slice_op)? {
                        ShapeSplit::Flush => {
                            ensure!(intersection_shape.is_none(), "multiple intersection shapes");
                            ev.log_value("intersection_shape", child);
                            intersection_shape = Some(child);
                        }
                        child_result @ ShapeSplit::ManifoldInside
                        | child_result @ ShapeSplit::ManifoldOutside => {
                            let is_inside = child_result == ShapeSplit::ManifoldInside;
                            let is_outside = child_result == ShapeSplit::ManifoldOutside;

                            let child_manifold = &self[child.id].manifold;
                            let shape_manifold = &self[shape].manifold;
                            let result = intersection_manifold
                                .which_side(child_manifold, shape_manifold)?
                                * child.sign;
                            // Is any part of the cut on the inside of `child`?
                            if result.is_any_inside {
                                self_boundary_of_inside.extend(is_inside.then_some(child.into()));
                                self_boundary_of_outside.extend(is_outside.then_some(child.into()));
                            } else {
                                ev.log("Child completely excludes cut. Exiting early ...");
                                return Ok(ShapeSplit::NonFlush {
                                    inside: is_inside.then_some(shape.into()),
                                    outside: is_outside.then_some(shape.into()),
                                    intersection_shape: None,
                                });
                            }
                        }
                        ShapeSplit::NonFlush {
                            inside,
                            outside,
                            intersection_shape,
                        } => {
                            self_boundary_of_inside.extend(inside);
                            self_boundary_of_outside.extend(outside);
                            intersection_boundary.extend(intersection_shape.map(|s| -s));
                        }
                    }
                }

                ev.log_set64("self_boundary_of_inside", &self_boundary_of_inside);
                ev.log_set64("self_boundary_of_outside", &self_boundary_of_outside);
                ev.log_set64("intersection_boundary", &intersection_boundary);
                ev.log_option("intersection_shape", intersection_shape);

                if let Some(shape) = &mut intersection_shape {
                    ev.log("`shape ∩ boundary(cut)` is nonempty (case 1)");

                    // There already exists an intersection shape, so either
                    // `shape ∩ cut` is empty or `shape ∩ ~cut` is empty.
                    ev.log_value("intersection_shape", *shape);
                    let sign_difference = self
                        .sign_difference(*shape, &intersection_manifold)?
                        .context(
                            "manifold of intersection shape does \
                         not match intersection manifold",
                        )?;
                    *shape = *shape * sign_difference;
                    match sign_difference {
                        Sign::Pos => self_boundary_of_inside.insert(*shape),
                        Sign::Neg => self_boundary_of_outside.insert(-*shape),
                    };
                } else {
                    ev.log("Simplifying boundary of intersection");
                    intersection_boundary = self
                        .simplify_shape_boundary(&intersection_manifold, intersection_boundary)?;

                    // Is `shape ∩ boundary(cut)` (`intersection_shape`) nonempty?
                    let is_intersection_nonempty = if !intersection_boundary.is_empty() {
                        // `boundary(shape ∩ boundary(cut))` is non-empty; in other
                        // words, `shape ∩ boundary(cut)` has a boundary.
                        ev.log("`shape ∩ boundary(cut)` is nonempty (case 2)");
                        true
                        // If `shape ∩ boundary(cut)` has no boundary, then it is
                        // either empty or the whole cut manifold (hence the next
                        // two cases).
                    } else if self
                        .shape_completely_contains_manifold(shape, &intersection_manifold)?
                    {
                        // `shape ∩ boundary(cut) = intersection_manifold ⊆ shape`
                        ev.log("`shape ∩ boundary(cut)` is nonempty (case 3)");
                        true
                    } else {
                        // None of the other conditions are met, so `shape ∩
                        // boundary(cut)` is empty.
                        ev.log("`shape ∩ boundary(cut)` is empty (case 4)");
                        false
                    };

                    if is_intersection_nonempty {
                        ev.log("`intersection_shape` does not yet exist, but should be nonempty");

                        // Construct the shape that is `shape ∩ boundary(cut)`.
                        let intersection =
                            self.add(Shape::new(intersection_manifold, intersection_boundary))?;
                        intersection_shape = Some(intersection);

                        // `shape ∩ boundary(cut)` is part of the boundary of `shape
                        // ∩ cut` and part of the boundary of `shape ∩ ~cut`.
                        self_boundary_of_inside.insert(intersection);
                        self_boundary_of_outside.insert(-intersection);
                    }
                }

                // At this point, we have finished computing `boundary(shape ∩
                // cut)` and `boundary(shape ∩ !cut)`. `shape.manifold` is split
                // by the cut, so if `boundary(shape ∩ cut)` is empty then
                // `shape ∩ cut` (`inside`) is empty. And ditto for `shape ∩
                // ~cut` (`outside`).

                // Construct the N-dimensional shape that is `self ∩ cut`
                let inside = if self_boundary_of_inside.is_empty() {
                    None
                } else {
                    ev.log("Constructing inside shape");
                    Some(self.add_subshape(shape, self_boundary_of_inside)?)
                };
                // Construct the N-dimensional shape that is `self ∩ ~cut`
                let outside = if self_boundary_of_outside.is_empty() {
                    None
                } else {
                    ev.log("Constructing outside shape");
                    Some(self.add_subshape(shape, self_boundary_of_outside)?)
                };

                ev.log_option("inside", inside);
                ev.log_option("outside", outside);
                ev.log_option("intersection_shape", intersection_shape);

                Ok(ShapeSplit::NonFlush {
                    inside,
                    outside,
                    intersection_shape,
                })
            }
        }
    }

    /// Returns whether `manifold` (which is assumed to be flush with the
    /// manifold of `shape`) is completely inside `shape`. (This includes the
    /// boundary of `shape`, not just its interior.)
    fn shape_completely_contains_manifold(
        &self,
        shape: ShapeId,
        manifold: &Manifold,
    ) -> Result<bool> {
        let shape_manifold = &self[shape].manifold;
        for boundary_elem in self[shape].boundary.iter() {
            let boundary_elem_manifold = &self[boundary_elem.id].manifold;
            let which_side =
                manifold.which_side(boundary_elem_manifold, shape_manifold)? * boundary_elem.sign;
            if which_side.is_any_outside {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// Returns true if `point` is inside or on the boundary of `shape` or false
    /// if it is strictly outside. Returns false if `point` is not flush with
    /// `shape`.
    pub fn shape_contains_point(&self, shape: ShapeId, point: &Point) -> Result<bool> {
        let shape_manifold = &self[shape].manifold;
        let which_side = shape_manifold.which_side_has_point(point, &self.space)?;
        if which_side != PointWhichSide::On {
            return Ok(false);
        }
        for boundary_elem in self[shape].boundary.iter() {
            let boundary_elem_manifold = &self[boundary_elem.id].manifold;
            let which_side = boundary_elem_manifold.which_side_has_point(point, shape_manifold)?
                * boundary_elem.sign;
            match which_side {
                PointWhichSide::On => return self.shape_contains_point(boundary_elem.id, point),
                PointWhichSide::Inside => continue,
                PointWhichSide::Outside => return Ok(false),
            }
        }

        Ok(true)
    }
    /// Returns true if `point` is inside. Returns false if `point` is not flush
    /// with `shape` or it is on the boundary.
    pub fn shape_interior_contains_point(&self, shape: ShapeId, point: &Point) -> Result<bool> {
        let shape_manifold = &self[shape].manifold;
        let which_side = shape_manifold.which_side_has_point(point, &self.space)?;
        if which_side != PointWhichSide::On {
            return Ok(false);
        }
        for boundary_elem in self[shape].boundary.iter() {
            let boundary_elem_manifold = &self[boundary_elem.id].manifold;
            let which_side = boundary_elem_manifold.which_side_has_point(point, shape_manifold)?
                * boundary_elem.sign;
            match which_side {
                PointWhichSide::On => return Ok(false),
                PointWhichSide::Inside => continue,
                PointWhichSide::Outside => return Ok(false),
            }
        }

        Ok(true)
    }

    fn simplify_shape_boundary(
        &mut self,
        manifold: &Manifold,
        boundary_set: Set64<ShapeRef>,
    ) -> Result<Set64<ShapeRef>> {
        if manifold.ndim()? == 1 {
            Ok(self
                .simplify_intervals_intersection(boundary_set.iter(), &manifold)
                .context("error simplifying boundary of 1D intersection")?
                .unwrap_or_else(Set64::new))
        } else {
            // Just remove duplicates (which `Set64` does automatically for us)
            // and cancel opposite signs.
            Ok(boundary_set
                .iter()
                .filter(|&elem| !boundary_set.contains(-elem))
                .collect())
        }
    }

    /// Simplifies a subset of a 1D manifold represented as the intersection of
    /// a set of intervals, where each interval is represented as a point pair.
    ///
    /// Returns `None` if the intersection is empty.
    fn simplify_intervals_intersection(
        &mut self,
        intervals: impl IntoIterator<Item = ShapeRef>,
        space: &Manifold,
    ) -> Result<Option<Set64<ShapeRef>>> {
        let ev = self
            .log
            .event("simplify_intervals", "Simplifying intervals intersection");
        ev.log_value("space", space);

        let mut simplified = Set64::new();
        for interval in intervals {
            ev.log_value("interval", interval);
            match self.incremental_simplify_intervals_intersection(&simplified, interval, space)? {
                Some(b) => simplified = b,
                None => return Ok(None),
            }
        }
        ev.log_set64("simplified", &simplified);
        Ok(Some(simplified))
    }
    /// Intersects a set of intervals with another interval, where each interval
    /// is represented as a point pair.
    ///
    /// Returns `None` if the intersection is empty.
    fn incremental_simplify_intervals_intersection(
        &mut self,
        existing_intervals: &Set64<ShapeRef>,
        mut new_interval: ShapeRef,
        space: &Manifold,
    ) -> Result<Option<Set64<ShapeRef>>> {
        let ev = self.log.event(
            "simplify_intervals_incremental",
            "Adding simplified intersecting intervals",
        );
        ev.log_set64("existing_intervals", existing_intervals);
        ev.log_value("new_interval", new_interval);
        ev.log_value("space", space);

        let mut simplified = Set64::new();
        for existing_interval in existing_intervals.iter() {
            // The intersection of intervals is the complement of the union of
            // the complements. (Negating a point pair corresponds to taking the
            // complement of an interval.)
            match self.try_merge_intervals(-existing_interval, -new_interval, space)? {
                MergedInterval::Old(shape) => new_interval = -shape,
                MergedInterval::New(manifold) => {
                    new_interval = self.add(Shape::whole_space(manifold.flip()?))?;
                }

                MergedInterval::WholeSpace => return Ok(None), // whole space is excluded; there's nothing left

                MergedInterval::NoIntersection => {
                    simplified.insert(existing_interval);
                }
            }
        }
        simplified.insert(new_interval);

        // Check that all points are unique.
        if cfg!(debug_assertions) {
            let mut verts = simplified
                .iter()
                .flat_map(|s| self.shape_to_point_pair(s).unwrap())
                .collect_vec();
            while let Some(v1) = verts.pop() {
                for v2 in &verts {
                    assert!(!approx_eq(&v1, v2))
                }
            }
        }

        Ok(Some(simplified))
    }
    /// If two intervals (including their boundaries) intersect at all, returns
    /// the combined interval. Otherwise returns `None`.
    fn try_merge_intervals(
        &self,
        interval1: ShapeRef,
        interval2: ShapeRef,
        space: &Manifold,
    ) -> Result<MergedInterval> {
        let ab = interval1;
        let pq = interval2;
        let [a, b] = self.shape_to_point_pair(ab)?;
        let [p, q] = self.shape_to_point_pair(pq)?;

        if approx_eq(&a, &p) && approx_eq(&b, &q) {
            // The intervals are the same.
            self.log.event("merge_int", "same");
            return Ok(MergedInterval::Old(ab));
        }

        let ab_has_p = self.closed_interval_contains_point(ab, &p, space)?;
        let ab_has_q = self.closed_interval_contains_point(ab, &q, space)?;
        let pq_has_a = self.closed_interval_contains_point(pq, &a, space)?;
        let pq_has_b = self.closed_interval_contains_point(pq, &b, space)?;
        let ab_has_pq = ab_has_p && ab_has_q;
        let pq_has_ab = pq_has_a && pq_has_b;

        if ab_has_pq && pq_has_ab {
            return Ok(MergedInterval::WholeSpace);
        }

        if ab_has_pq {
            return Ok(MergedInterval::Old(ab));
        }

        if pq_has_ab {
            return Ok(MergedInterval::Old(pq));
        }

        let start = if ab_has_p {
            a
        } else if pq_has_a {
            p
        } else {
            return Ok(MergedInterval::NoIntersection);
        };

        let end = if ab_has_q {
            b
        } else if pq_has_b {
            q
        } else {
            return Ok(MergedInterval::NoIntersection);
        };

        Ok(MergedInterval::New(Manifold::new_point_pair(
            &start, &end, space,
        )?))
    }
    /// Returns whether an interval (**including** its boundary) contains a
    /// point.
    fn closed_interval_contains_point(
        &self,
        interval: impl SignedManifold,
        point: &Point,
        space: &Manifold,
    ) -> Result<bool> {
        let interval_manifold = interval.get_manifold_from(self)?;
        let which_side = interval_manifold.which_side_has_point(point, space)? * interval.sign();
        self.log.event("interval_result", format!("{which_side:?}"));
        Ok(which_side != PointWhichSide::Outside)
    }
    /// Returns the pair of points represented by a 0D manifold.
    fn shape_to_point_pair(&self, shape: impl SignedManifold) -> Result<[Point; 2]> {
        let [a, b] = shape.get_manifold_from(self)?.to_point_pair()?;
        match shape.sign() {
            Sign::Pos => Ok([a, b]),
            Sign::Neg => Ok([b, a]),
        }
    }

    fn display_shape(
        &self,
        f: &mut fmt::Formatter<'_>,
        shape: ShapeRef,
        sign: Sign,
        indent: u8,
    ) -> fmt::Result {
        for _ in 0..indent {
            write!(f, "  ")?;
        }
        write!(f, "{}#{:<5}", shape.sign, shape.id.0)?;
        if let Ok(m) = self.signed_manifold_of_shape(shape) {
            write!(f, "{m}")?;
            if let Some(m) = self.get_metadata(shape) {
                write!(f, " (in={m})")?;
            }
            if let Some(m) = self.get_metadata(-shape) {
                write!(f, " (out={m})")?;
            }
            writeln!(f)?;
        }
        for child in self[shape.id].boundary.iter() {
            self.display_shape(f, child, shape.sign * sign, indent + 1)?;
        }
        Ok(())
    }
}

/// Parameters for cutting a bunch of shapes.
#[derive(Debug, Clone)]
pub struct CutParams {
    /// Closed, oriented manifold along which to cut.
    pub cut: Manifold,
    /// What to do with the shapes on the "inside" of the cut.
    pub inside: CutOp,
    /// What to do with the shapes on the "outside" of the cut.
    pub outside: CutOp,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CutOp {
    Remove,
    Keep(Option<ShapeMetadata>),
}
impl Default for CutOp {
    fn default() -> Self {
        CutOp::Keep(None)
    }
}
impl fmt::Display for CutOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CutOp::Remove => write!(f, "REMOVE"),
            CutOp::Keep(None) => write!(f, "KEEP"),
            CutOp::Keep(Some(metadata)) => write!(f, "KEEP (data = {metadata})"),
        }
    }
}
impl CutOp {
    fn metadata(self) -> Option<u16> {
        match self {
            CutOp::Remove => None,
            CutOp::Keep(metadata) => metadata,
        }
    }
}

/// In-progress slicing operation.
#[derive(Debug)]
struct SliceOperation {
    /// Closed, oriented manifold that divides the entire space into "inside"
    /// and "outside."
    divider: Manifold,
    /// Cache of the result of splitting individual shapes.
    results_cache: AHashMap<ShapeId, ShapeSplit>,

    /// Metadata to attach to the inside side of the shape.
    inside_metadata: Option<ShapeMetadata>,
    /// Metadata to attach to the outside side of the shape.
    outside_metadata: Option<ShapeMetadata>,
}
impl SliceOperation {
    fn new(cut: CutParams) -> Self {
        Self {
            divider: cut.cut,
            results_cache: AHashMap::new(),

            inside_metadata: cut.inside.metadata(),
            outside_metadata: cut.outside.metadata(),
        }
    }
}

/// Result of splitting an N-dimensional shape by a slicing manifold.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ShapeSplit {
    /// The shape's manifold is flush with the slicing manifold.
    Flush,
    /// The shape's manifold is completely on the inside of the slice.
    ManifoldInside,
    /// The shape's manifold is completely on the outside of the slice.
    ManifoldOutside,
    /// The shape's manifold intersects the slice but is not flush.
    NonFlush {
        /// N-dimensional portion of the shape that is inside the slice, if any.
        /// If this is the whole shape, then `outside` must be `None` (but
        /// `intersection_shape` may be `Some`).
        inside: Option<ShapeRef>,
        /// N-dimensional portion of the shape that is outside the slice, if
        /// any. If this is the whole shape, then `inside` must be `None` (but
        /// `intersection_shape` may be `Some`).
        outside: Option<ShapeRef>,

        /// (N-1)-dimensional intersection of the shape with the slicing
        /// manifold, if any. If `inside` and `outside` are both `Some`, then
        /// this must be `Some`.
        intersection_shape: Option<ShapeRef>,
    },
}
impl fmt::Display for ShapeSplit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShapeSplit::Flush => write!(f, "Flush"),
            ShapeSplit::ManifoldInside => write!(f, "ManifoldInside"),
            ShapeSplit::ManifoldOutside => write!(f, "ManifoldOutside"),
            ShapeSplit::NonFlush {
                inside,
                outside,
                intersection_shape,
            } => {
                write!(
                    f,
                    "NonFlush {{ inside: {}, outside: {}, intersection_shape: {} }}",
                    inside.map_or_else(|| "<none>".to_string(), |x| x.to_string()),
                    outside.map_or_else(|| "<none>".to_string(), |x| x.to_string()),
                    intersection_shape.map_or_else(|| "<none>".to_string(), |x| x.to_string()),
                )
            }
        }
    }
}
impl Neg for ShapeSplit {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        fn negate_option_shape_ref(r: &mut Option<ShapeRef>) {
            if let Some(r) = r {
                *r = -*r
            }
        }

        if let ShapeSplit::NonFlush {
            inside,
            outside,
            intersection_shape,
        } = &mut self
        {
            negate_option_shape_ref(inside);
            negate_option_shape_ref(outside);
            negate_option_shape_ref(intersection_shape);
        }

        self
    }
}
hypermath::impl_mul_sign!(impl Mul<Sign> for ShapeRef);
hypermath::impl_mulassign_sign!(impl MulAssign<Sign> for ShapeRef);

trait SignedManifold {
    fn get_manifold_from<'a>(&'a self, arena: &'a ShapeArena) -> Result<&'a Manifold>;
    fn sign(&self) -> Sign;
}
impl SignedManifold for ShapeRef {
    fn get_manifold_from<'a>(&'a self, arena: &'a ShapeArena) -> Result<&'a Manifold> {
        Ok(&arena[self.id].manifold)
    }
    fn sign(&self) -> Sign {
        self.sign
    }
}
impl SignedManifold for Manifold {
    fn get_manifold_from<'a>(&'a self, _arena: &'a ShapeArena) -> Result<&'a Manifold> {
        Ok(self)
    }
    fn sign(&self) -> Sign {
        Sign::Pos
    }
}
impl SignedManifold for &Manifold {
    fn get_manifold_from<'a>(&'a self, _arena: &'a ShapeArena) -> Result<&'a Manifold> {
        Ok(self)
    }
    fn sign(&self) -> Sign {
        Sign::Pos
    }
}

enum MergedInterval {
    Old(ShapeRef),
    New(Manifold),
    WholeSpace,
    NoIntersection,
}