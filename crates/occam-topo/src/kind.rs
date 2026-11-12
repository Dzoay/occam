//! Source: original
//! Entity kinds as types, so a face's id cannot go where an edge's is wanted, and ids of slot and generation (S3-P3).

use core::fmt::Debug;
use core::marker::PhantomData;
use core::num::NonZeroU32;

use occam_report::{EntityKind, EntityRef};

use crate::records::{
    BodyRecord, CurveRecord, EdgeRecord, EdgeUseRecord, FaceRecord, LoopRecord, ParamCurveRecord, ShellRecord,
    SurfaceRecord, VertexRecord, WireRecord,
};
use crate::snapshot::Model;
use crate::table::Table;

pub(crate) mod sealed {
    use occam_report::EntityRef;

    use super::Kind;
    use crate::snapshot::Model;
    use crate::table::Table;

    /// What only this crate may do with a kind: reach its table, which is how the edit stays the one way to change a
    /// model, and list the ids its records link to.
    pub trait Sealed {
        /// Its table in `model`.
        fn table(model: &Model) -> &Table<<Self as Kind>::Record>
        where
            Self: Kind;

        /// Its table in `model`, to write.
        fn table_mut(model: &mut Model) -> &mut Table<<Self as Kind>::Record>
        where
            Self: Kind;

        /// Every entity `record` links to, in the order its fields name them.
        fn links(record: &<Self as Kind>::Record, out: &mut Vec<EntityRef>)
        where
            Self: Kind;
    }
}

/// An entity kind as a type. Only this crate's kinds implement it.
pub trait Kind: sealed::Sealed + Copy + Eq + Ord + Debug + Send + Sync + 'static {
    /// The kind in a report.
    const ENTITY: EntityKind;

    /// What an edit writes for an entity of this kind.
    type Record: Clone + Debug + PartialEq + Send + Sync + 'static;
}

/// An entity: its slot in its kind's table and the generation of that slot (S3-P3), with its kind as the type. The
/// generation changes only when a slot is reused, so an entity changed in place keeps its id. Ids order by slot, then
/// generation.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id<K: Kind> {
    slot: u32,
    generation: NonZeroU32,
    kind: PhantomData<fn() -> K>,
}

impl<K: Kind> Id<K> {
    /// The id of `slot` at `generation`.
    pub(crate) const fn new(slot: u32, generation: NonZeroU32) -> Self {
        Self { slot, generation, kind: PhantomData }
    }

    /// Its slot in its kind's table.
    #[must_use]
    pub const fn slot(self) -> u32 {
        self.slot
    }

    /// How many times its slot had been used when it was given.
    #[must_use]
    pub const fn generation(self) -> NonZeroU32 {
        self.generation
    }

    /// The same entity as a report names it: `edge:41/1`.
    #[must_use]
    pub const fn entity(self) -> EntityRef {
        EntityRef::new(K::ENTITY, self.slot, self.generation.get())
    }
}

impl<K: Kind> Debug for Id<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.entity().text())
    }
}

/// Declares each kind's marker type and joins it to its record and its table.
macro_rules! kinds {
    ($($(#[$meta:meta])* $marker:ident => $entity:ident, $record:ty, $field:ident;)*) => {
        $(
            $(#[$meta])*
            #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
            pub struct $marker;

            impl sealed::Sealed for $marker {
                fn table(model: &Model) -> &Table<$record> {
                    &model.$field
                }

                fn table_mut(model: &mut Model) -> &mut Table<$record> {
                    &mut model.$field
                }

                fn links(record: &$record, out: &mut Vec<EntityRef>) {
                    record.links(out);
                }
            }

            impl Kind for $marker {
                const ENTITY: EntityKind = EntityKind::$entity;
                type Record = $record;
            }
        )*
    };
}

kinds! {
    /// A body: a solid, a sheet or a wire.
    Body => Body, BodyRecord, bodies;
    /// A shell: faces joined along their edges.
    Shell => Shell, ShellRecord, shells;
    /// A face: a bounded piece of one surface.
    Face => Face, FaceRecord, faces;
    /// A loop of edge uses bounding a face.
    Loop => Loop, LoopRecord, loops;
    /// A chain of edge uses that bounds no face.
    Wire => Wire, WireRecord, wires;
    /// One use of an edge by a loop or a wire, with its direction.
    EdgeUse => EdgeUse, EdgeUseRecord, uses;
    /// An edge: a bounded piece of one curve between two vertices.
    Edge => Edge, EdgeRecord, edges;
    /// A vertex: a point where edges end.
    Vertex => Vertex, VertexRecord, vertices;
    /// A surface that faces lie on.
    SurfaceGeom => Surface, SurfaceRecord, surfaces;
    /// A curve that edges lie on.
    CurveGeom => Curve, CurveRecord, curves;
    /// A curve in a face's parameters, the image of an edge there.
    ParamCurveGeom => ParamCurve, ParamCurveRecord, pcurves;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id<K: Kind>(slot: u32, generation: u32) -> Id<K> {
        Id::new(slot, NonZeroU32::new(generation).unwrap())
    }

    #[test]
    fn an_id_is_named_by_its_kind_slot_and_generation() {
        assert_eq!(id::<Edge>(41, 1).entity().text(), "edge:41/1");
        assert_eq!(id::<EdgeUse>(0, 7).entity(), EntityRef::new(EntityKind::EdgeUse, 0, 7));
        assert_eq!(id::<ParamCurveGeom>(3, 2).entity().text(), "param-curve:3/2");
        assert_eq!(format!("{:?}", id::<Face>(12, 3)), "face:12/3");
        assert_eq!((id::<Vertex>(5, 2).slot(), id::<Vertex>(5, 2).generation().get()), (5, 2));
    }

    #[test]
    fn ids_compare_by_slot_then_generation() {
        assert!(id::<Face>(1, 9) < id::<Face>(2, 1));
        assert!(id::<Face>(2, 1) < id::<Face>(2, 2));
        assert_eq!(id::<Face>(2, 2), id::<Face>(2, 2));
        assert_ne!(id::<Face>(2, 2), id::<Face>(2, 3));
        assert_eq!(id::<Face>(2, 2).partial_cmp(&id::<Face>(2, 3)), Some(core::cmp::Ordering::Less));
        assert_eq!(id::<Face>(4, 1).clone(), id::<Face>(4, 1));
    }
}
