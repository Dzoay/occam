//! Source: original
//! How the checks read a model: a record at a time, counted in tests so that a test can show that what a commit reads
//! follows the edit and not the model (P1), and walks along the links that make lists and rings.

use core::num::NonZeroU32;
use std::collections::BTreeSet;

use occam_report::EntityRef;

use crate::kind::{Body, Edge, EdgeUse, Face, Id, Kind, Loop, Shell, Vertex};
use crate::records::{Around, BodyKind, EdgeRecord, End, UseOwner};
use crate::snapshot::Model;

#[cfg(test)]
thread_local! {
    static READS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Counts one record read, in tests.
pub(crate) fn note() {
    #[cfg(test)]
    READS.with(|reads| reads.set(reads.get().saturating_add(1)));
}

/// How many records the checks have read on this thread.
#[cfg(test)]
pub(crate) fn reads() -> usize {
    READS.with(core::cell::Cell::get)
}

/// The live record of `id`, counted.
pub(crate) fn get<K: Kind>(model: &Model, id: Id<K>) -> Option<&K::Record> {
    note();
    K::table(model).live(id.slot(), id.generation())
}

/// The id of kind `K` that `entity` names, if it names one of that kind.
pub(crate) fn typed<K: Kind>(entity: EntityRef) -> Option<Id<K>> {
    let generation = NonZeroU32::new(entity.generation).filter(|_| entity.kind == K::ENTITY)?;
    Some(Id::new(entity.slot, generation))
}

/// Where a walk along links went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Walk<T> {
    /// Back to where it began: every item once, the first first.
    Ring(Vec<T>),
    /// To an end: every item once, in order.
    End(Vec<T>),
    /// To an item it had passed other than the first, or to one it could not read, named last.
    Broken(Vec<T>, T),
}

impl<T: Copy> Walk<T> {
    /// Every item it passed, the one it broke at included.
    pub(crate) fn items(&self) -> Vec<T> {
        match self {
            Self::Ring(items) | Self::End(items) => items.clone(),
            Self::Broken(items, at) => items.iter().copied().chain([*at]).collect(),
        }
    }

    /// The items of a sound list: one that ends, or comes back to its first if `ring`.
    pub(crate) fn sound(&self, ring: bool) -> Option<&[T]> {
        match self {
            Self::Ring(items) if ring => Some(items),
            Self::End(items) if !ring => Some(items),
            Self::Ring(_) | Self::End(_) | Self::Broken(..) => None,
        }
    }
}

/// Walks from `first` by `next`, which gives `Some(Some(item))` for the next item, `Some(None)` at an end, and `None`
/// where the item it is given cannot be read. It stops at the first item passed twice, so it always ends.
pub(crate) fn walk<T: Copy + Ord>(first: T, mut next: impl FnMut(T) -> Option<Option<T>>) -> Walk<T> {
    let mut items = vec![first];
    let mut seen = BTreeSet::from([first]);
    let mut at = first;
    loop {
        let Some(step) = next(at) else { return Walk::Broken(items, at) };
        let Some(following) = step else { return Walk::End(items) };
        if following == first {
            return Walk::Ring(items);
        }
        if !seen.insert(following) {
            return Walk::Broken(items, following);
        }
        items.push(following);
        at = following;
    }
}

/// The list from `first` by a `next` field, if there is a first.
pub(crate) fn list<K: Kind>(
    model: &Model,
    first: Option<Id<K>>,
    next: impl Fn(&K::Record) -> Option<Id<K>>,
) -> Walk<Id<K>> {
    first.map_or(Walk::End(Vec::new()), |first| walk(first, |at| get(model, at).map(&next)))
}

/// The uses of an edge, round the ring of `radial` links from its first.
pub(crate) fn radial(model: &Model, edge: &EdgeRecord) -> Walk<Id<EdgeUse>> {
    list(model, edge.first_use, |u| Some(u.radial))
}

/// The vertex at `end` of an edge.
pub(crate) const fn end_vertex(edge: &EdgeRecord, end: End) -> Id<Vertex> {
    match end {
        End::Start => edge.ends.0,
        End::Finish => edge.ends.1,
    }
}

/// The places round a vertex's star, from `start` by each edge's `around` link at that end.
pub(crate) fn star(model: &Model, start: Around) -> Walk<Around> {
    walk(start, |place| {
        get(model, place.edge).map(|e| {
            Some(match place.end {
                End::Start => e.around.0,
                End::Finish => e.around.1,
            })
        })
    })
}

/// The uses of a loop or wire, from its first along `next`.
pub(crate) fn uses_from(model: &Model, first: Id<EdgeUse>) -> Walk<Id<EdgeUse>> {
    walk(first, |at| get(model, at).map(|u| u.next))
}

/// The first use of a loop or wire.
pub(crate) fn first_use(model: &Model, owner: UseOwner) -> Option<Id<EdgeUse>> {
    match owner {
        UseOwner::Loop(l) => get(model, l).map(|r| r.first),
        UseOwner::Wire(w) => get(model, w).map(|r| r.first),
    }
}

/// The vertices a use runs from and to, in its own direction.
pub(crate) fn use_ends(model: &Model, id: Id<EdgeUse>) -> Option<(Id<Vertex>, Id<Vertex>)> {
    let record = get(model, id)?;
    let (start, finish) = get(model, record.edge)?.ends;
    Some(if record.reversed { (finish, start) } else { (start, finish) })
}

/// The face a loop bounds.
pub(crate) fn loop_face(model: &Model, id: Id<Loop>) -> Option<Id<Face>> {
    get(model, id).map(|r| r.face)
}

/// The shell of the face a loop bounds.
pub(crate) fn loop_shell(model: &Model, id: Id<Loop>) -> Option<Id<Shell>> {
    get(model, loop_face(model, id)?).map(|f| f.shell)
}

/// The shell a use lies in, if its owner is a loop.
pub(crate) fn use_shell(model: &Model, id: Id<EdgeUse>) -> Option<Id<Shell>> {
    match get(model, id)?.owner {
        UseOwner::Loop(l) => loop_shell(model, l),
        UseOwner::Wire(_) => None,
    }
}

/// The body a use belongs to, through its loop's face and shell or its wire.
pub(crate) fn use_body(model: &Model, id: Id<EdgeUse>) -> Option<Id<Body>> {
    match get(model, id)?.owner {
        UseOwner::Loop(l) => get(model, loop_shell(model, l)?).map(|s| s.body),
        UseOwner::Wire(w) => get(model, w).map(|w| w.body),
    }
}

/// The kind of a body.
pub(crate) fn body_kind(model: &Model, id: Id<Body>) -> Option<BodyKind> {
    get(model, id).map(|b| b.kind)
}

/// The shells an edge's uses lie in, each once.
pub(crate) fn edge_shells(model: &Model, id: Id<Edge>) -> BTreeSet<Id<Shell>> {
    let Some(edge) = get(model, id) else { return BTreeSet::new() };
    radial(model, edge).items().into_iter().filter_map(|u| use_shell(model, u)).collect()
}

/// The edges round a vertex's star, each once.
pub(crate) fn vertex_edges(model: &Model, id: Id<Vertex>) -> BTreeSet<Id<Edge>> {
    let star_place = get(model, id).and_then(|v| v.star);
    star_place.map(|place| star(model, place).items().into_iter().map(|p| p.edge).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(links: &[(u8, Option<u8>)]) -> impl FnMut(u8) -> Option<Option<u8>> + '_ {
        |at| links.iter().find(|(from, _)| *from == at).map(|(_, to)| *to)
    }

    #[test]
    fn a_walk_says_whether_it_came_round_ended_or_broke_and_where() {
        assert_eq!(walk(1, steps(&[(1, Some(2)), (2, Some(3)), (3, Some(1))])), Walk::Ring(vec![1, 2, 3]));
        assert_eq!(walk(1, steps(&[(1, Some(2)), (2, None)])), Walk::End(vec![1, 2]));
        assert_eq!(walk(1, steps(&[(1, Some(2)), (2, Some(3)), (3, Some(2))])), Walk::Broken(vec![1, 2, 3], 2));
        assert_eq!(walk(1, steps(&[(1, Some(2))])), Walk::Broken(vec![1, 2], 2), "2 cannot be read");
        assert_eq!(walk(1, steps(&[(1, Some(1))])), Walk::Ring(vec![1]));
        let broken = Walk::Broken(vec![1, 2], 3);
        assert_eq!((broken.items(), broken.sound(true), broken.sound(false)), (vec![1, 2, 3], None, None));
        let ring = Walk::Ring(vec![4, 5]);
        assert_eq!((ring.sound(true), ring.sound(false)), (Some(&[4, 5][..]), None));
        assert_eq!(Walk::End(vec![6]).sound(false), Some(&[6][..]));
    }
}
