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
    pub(crate) fn items(self) -> Vec<T> {
        match self {
            Self::Ring(items) | Self::End(items) => items,
            Self::Broken(mut items, at) => {
                items.push(at);
                items
            }
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

    /// As [`Walk::sound`], giving up the items rather than lending them.
    pub(crate) fn into_sound(self, ring: bool) -> Option<Vec<T>> {
        self.sound(ring).is_some().then(|| self.items())
    }
}

/// How many items a walk looks along for one it has passed before it keeps a set of them instead. The loops, rings and
/// stars a check walks hold a few items, and looking along a few costs less than making a set.
const LINEAR: usize = 32;

/// How many items a walk makes room for at first: as many as most loops, rings and stars hold.
const ROOM: usize = 8;

#[cfg(test)]
thread_local! {
    static SETS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// How many walks on this thread have kept a set of what they passed.
#[cfg(test)]
fn sets() -> usize {
    SETS.with(core::cell::Cell::get)
}

/// Walks from `first` by `next`, which gives `Some(Some(item))` for the next item, `Some(None)` at an end, and `None`
/// where the item it is given cannot be read. It stops at the first item passed twice, so it always ends. It looks for
/// that item along those it passed, until it has passed [`LINEAR`], and then in a set of them it keeps from there on.
pub(crate) fn walk<T: Copy + Ord>(first: T, mut next: impl FnMut(T) -> Option<Option<T>>) -> Walk<T> {
    let mut items = Vec::with_capacity(ROOM);
    items.push(first);
    let mut seen: Option<BTreeSet<T>> = None;
    let mut at = first;
    loop {
        let Some(step) = next(at) else { return Walk::Broken(items, at) };
        let Some(following) = step else { return Walk::End(items) };
        if following == first {
            return Walk::Ring(items);
        }
        let passed = match &mut seen {
            Some(set) => !set.insert(following),
            None => items.contains(&following),
        };
        if passed {
            return Walk::Broken(items, following);
        }
        if seen.is_none() && items.len() >= LINEAR {
            seen = Some(kept(&items, following));
        }
        items.push(following);
        at = following;
    }
}

/// The set a long walk keeps of the items it passed and the one it has come to, counted in tests.
fn kept<T: Copy + Ord>(items: &[T], following: T) -> BTreeSet<T> {
    #[cfg(test)]
    SETS.with(|sets| sets.set(sets.get().saturating_add(1)));
    items.iter().copied().chain([following]).collect()
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
        assert_eq!((broken.sound(true), broken.sound(false)), (None, None));
        assert_eq!(broken.items(), vec![1, 2, 3]);
        let ring = Walk::Ring(vec![4, 5]);
        assert_eq!((ring.sound(true), ring.sound(false)), (Some(&[4, 5][..]), None));
        assert_eq!(Walk::End(vec![6]).sound(false), Some(&[6][..]));
    }

    #[test]
    fn a_sound_list_gives_up_its_items_and_any_other_none() {
        let both = |walk: Walk<u8>| (walk.clone().into_sound(true), walk.into_sound(false));
        assert_eq!(both(Walk::Ring(vec![4, 5])), (Some(vec![4, 5]), None));
        assert_eq!(both(Walk::End(vec![6])), (None, Some(vec![6])));
        assert_eq!(both(Walk::Broken(vec![1], 2)), (None, None));
        assert_eq!(both(Walk::End(Vec::new())), (None, Some(Vec::new())));
    }

    /// A walk from 0 along `n` items, each to the next, then from the last to `last`.
    fn chain(n: u8, last: Option<u8>) -> impl FnMut(u8) -> Option<Option<u8>> {
        move |at| Some(at.checked_add(1).filter(|&next| next < n).or(last))
    }

    /// What a walk along `chain(n, last)` gives, and how many sets of what it passed it kept.
    fn walked(n: u8, last: Option<u8>) -> (Walk<u8>, usize) {
        let before = sets();
        let walk = walk(0, chain(n, last));
        (walk, sets().saturating_sub(before))
    }

    #[test]
    fn a_walk_looks_along_what_it_passed_until_that_is_long_and_then_keeps_a_set_of_it() {
        let upto = |n: u8| (0..n).collect::<Vec<_>>();
        // Up to `LINEAR` items, it looks along them, wherever it ends, comes round or breaks.
        assert_eq!(walked(32, None), (Walk::End(upto(32)), 0));
        assert_eq!(walked(32, Some(0)), (Walk::Ring(upto(32)), 0));
        assert_eq!(walked(32, Some(5)), (Walk::Broken(upto(32), 5), 0));
        assert_eq!(walked(32, Some(31)), (Walk::Broken(upto(32), 31), 0));
        // Passing one more, it keeps a set, which holds what it passed before, the one that made it, and what came after.
        assert_eq!(walked(33, None), (Walk::End(upto(33)), 1));
        assert_eq!(walked(33, Some(0)), (Walk::Ring(upto(33)), 1));
        assert_eq!(walked(40, Some(5)), (Walk::Broken(upto(40), 5), 1));
        assert_eq!(walked(40, Some(32)), (Walk::Broken(upto(40), 32), 1));
        assert_eq!(walked(40, Some(39)), (Walk::Broken(upto(40), 39), 1));
        assert_eq!(walked(200, Some(0)), (Walk::Ring(upto(200)), 1));
    }
}
