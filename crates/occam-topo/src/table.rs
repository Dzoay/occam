//! Source: original
//! One kind's records in copy-on-write chunks of [`CHUNK`] (S3-P3). Cloning a table shares every chunk, and the first
//! write to a shared chunk copies that chunk alone (path copying, Driscoll, Sarnak, Sleator and Tarjan, 1989). Free slots
//! are reused lowest first, so the ids a table gives depend only on the table.

use core::num::NonZeroU32;
use std::sync::Arc;

/// Records a chunk holds: 1,024 suits edits that scatter, as a boolean's do (S3-P3: 14 µs an edit, against 41 with
/// chunks of 4,096).
pub(crate) const CHUNK: usize = 1024;

/// What a slot holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum State<R> {
    /// Nothing: free to reuse, unless its generation can grow no further.
    Free,
    /// Given out by an edit for a record written later.
    Reserved,
    /// A live record.
    Live(R),
}

/// A slot: the generation of its latest entity, and what it holds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slot<R> {
    generation: u32,
    state: State<R>,
}

impl<R> Slot<R> {
    /// What it holds.
    pub(crate) const fn state(&self) -> &State<R> {
        &self.state
    }

    /// The generation of its latest entity.
    pub(crate) const fn generation(&self) -> u32 {
        self.generation
    }

    /// Whether an edit may give it out again: free, with a generation that can still grow.
    const fn reusable(&self) -> bool {
        matches!(self.state, State::Free) && self.generation < u32::MAX
    }
}

/// One kind's records. `pub` only so that the sealed kind trait can name it; nothing outside the crate can reach one.
#[derive(Clone, Debug, PartialEq)]
pub struct Table<R> {
    chunks: Vec<Arc<Vec<Slot<R>>>>,
    /// How many reusable slots each chunk has.
    free: Vec<u32>,
    /// No chunk before this one has a reusable slot.
    lowest: usize,
    /// The most slots the table may have. Slot `u32::MAX` is never given, so an id that names it is never live.
    most: u32,
}

/// The chunk and the place in it of `slot`.
fn place(slot: u32) -> Option<(usize, usize)> {
    let slot = usize::try_from(slot).ok()?;
    Some((slot / CHUNK, slot % CHUNK))
}

impl<R: Clone> Table<R> {
    /// An empty table.
    pub(crate) const fn new() -> Self {
        Self::with_most(u32::MAX)
    }

    /// An empty table that may hold at most `most` slots, so that a test can fill one.
    pub(crate) const fn with_most(most: u32) -> Self {
        Self { chunks: Vec::new(), free: Vec::new(), lowest: 0, most }
    }

    /// The slot numbered `slot`, if the table has one.
    pub(crate) fn slot(&self, slot: u32) -> Option<&Slot<R>> {
        let (chunk, at) = place(slot)?;
        self.chunks.get(chunk)?.get(at)
    }

    /// The live record of `slot` at `generation`.
    pub(crate) fn live(&self, slot: u32, generation: NonZeroU32) -> Option<&R> {
        let found = self.slot(slot).filter(|s| s.generation == generation.get())?;
        match &found.state {
            State::Live(record) => Some(record),
            State::Free | State::Reserved => None,
        }
    }

    /// The generation of the live record in `slot`, if it holds one.
    pub(crate) fn live_generation(&self, slot: u32) -> Option<u32> {
        self.slot(slot).filter(|s| matches!(s.state, State::Live(_))).map(Slot::generation)
    }

    /// Every slot in order, with its number.
    pub(crate) fn slots(&self) -> impl Iterator<Item = (u32, &Slot<R>)> {
        self.chunks.iter().flat_map(|chunk| chunk.iter()).zip(0_u32..).map(|(slot, n)| (n, slot))
    }

    /// Every live record in slot order, with its slot and generation.
    pub(crate) fn records(&self) -> impl Iterator<Item = (u32, NonZeroU32, &R)> {
        self.slots().filter_map(|(n, slot)| match &slot.state {
            State::Live(record) => Some((n, NonZeroU32::new(slot.generation)?, record)),
            State::Free | State::Reserved => None,
        })
    }

    /// The slot numbered `slot`, to write: its chunk is copied first if a snapshot shares it.
    fn slot_mut(&mut self, slot: u32) -> Option<&mut Slot<R>> {
        let (chunk, at) = place(slot)?;
        Arc::make_mut(self.chunks.get_mut(chunk)?).get_mut(at)
    }

    /// Reserves the lowest reusable slot, or a new one at the end, and gives its number and new generation. `None` if
    /// the table is full.
    pub(crate) fn reserve(&mut self) -> Option<(u32, NonZeroU32)> {
        self.reuse().or_else(|| self.append())
    }

    /// Reserves the lowest reusable slot, its generation one more than before.
    fn reuse(&mut self) -> Option<(u32, NonZeroU32)> {
        let chunk = (self.lowest..self.chunks.len()).find(|&c| self.free.get(c).is_some_and(|&n| n > 0));
        let Some(chunk) = chunk else {
            self.lowest = self.chunks.len();
            return None;
        };
        self.lowest = chunk;
        let at = self.chunks.get(chunk)?.iter().position(Slot::reusable)?;
        let count = self.free.get_mut(chunk)?;
        *count = count.saturating_sub(1);
        let slot = u32::try_from(chunk.checked_mul(CHUNK)?.checked_add(at)?).ok()?;
        let entry = self.slot_mut(slot)?;
        let generation = NonZeroU32::new(entry.generation.checked_add(1)?)?;
        *entry = Slot { generation: generation.get(), state: State::Reserved };
        Some((slot, generation))
    }

    /// Reserves a new slot at the end, at generation 1.
    fn append(&mut self) -> Option<(u32, NonZeroU32)> {
        let count = self.chunks.len().saturating_sub(1).saturating_mul(CHUNK);
        let count = count.saturating_add(self.chunks.last().map_or(0, |chunk| chunk.len()));
        let slot = u32::try_from(count).ok().filter(|&n| n < self.most)?;
        if self.chunks.last().is_none_or(|chunk| chunk.len() >= CHUNK) {
            self.chunks.push(Arc::new(Vec::with_capacity(CHUNK)));
            self.free.push(0);
        }
        Arc::make_mut(self.chunks.last_mut()?).push(Slot { generation: 1, state: State::Reserved });
        Some((slot, NonZeroU32::MIN))
    }

    /// Writes `record` into `slot`, which must hold generation `generation`, reserved or live. `false` if it does not.
    pub(crate) fn put(&mut self, slot: u32, generation: NonZeroU32, record: R) -> bool {
        let Some(entry) = self.slot_mut(slot).filter(|s| s.generation == generation.get()) else { return false };
        if matches!(entry.state, State::Free) {
            return false;
        }
        entry.state = State::Live(record);
        true
    }

    /// Frees `slot`, which must hold generation `generation`, reserved or live. `false` if it does not.
    pub(crate) fn free(&mut self, slot: u32, generation: NonZeroU32) -> bool {
        let Some(entry) = self.slot_mut(slot).filter(|s| s.generation == generation.get()) else { return false };
        if matches!(entry.state, State::Free) {
            return false;
        }
        entry.state = State::Free;
        let reusable = entry.reusable();
        let Some((chunk, _)) = place(slot) else { return true };
        if reusable && let Some(count) = self.free.get_mut(chunk) {
            *count = count.saturating_add(1);
            self.lowest = self.lowest.min(chunk);
        }
        true
    }

    /// How many chunks this table shares with `other`.
    #[cfg(test)]
    #[expect(clippy::disallowed_methods, reason = "a test that chunks are shared compares them by address")]
    pub(crate) fn shared_chunks(&self, other: &Self) -> usize {
        self.chunks.iter().zip(&other.chunks).filter(|(a, b)| Arc::ptr_eq(a, b)).count()
    }

    /// How many chunks it has.
    #[cfg(test)]
    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Sets the generation of a slot, so that a test can reach the last one.
    #[cfg(test)]
    pub(crate) fn set_generation(&mut self, slot: u32, generation: u32) {
        if let Some(entry) = self.slot_mut(slot) {
            entry.generation = generation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(n: u32) -> Table<u32> {
        let mut table = Table::new();
        for k in 0..n {
            let (slot, generation) = table.reserve().unwrap();
            assert_eq!((slot, generation.get()), (k, 1));
            assert!(table.put(slot, generation, k));
        }
        table
    }

    fn one() -> NonZeroU32 {
        NonZeroU32::MIN
    }

    #[test]
    fn slots_are_given_in_order_across_chunk_boundaries() {
        let table = filled(2050);
        assert_eq!(table.chunk_count(), 3);
        for slot in [0, 1023, 1024, 1025, 2047, 2048, 2049] {
            assert_eq!(table.live(slot, one()), Some(&slot), "slot {slot}");
        }
        assert_eq!(table.live(2050, one()), None);
        assert!(table.records().all(|(slot, generation, record)| slot == *record && generation == one()));
        assert_eq!(table.records().count(), 2050);
    }

    #[test]
    fn freed_slots_are_reused_lowest_first_with_the_next_generation() {
        let mut table = filled(2100);
        for slot in [2099, 1024, 5, 1023] {
            assert!(table.free(slot, one()));
        }
        let given: Vec<(u32, u32)> = (0..5).map(|_| table.reserve().map(|(s, g)| (s, g.get())).unwrap()).collect();
        assert_eq!(given, [(5, 2), (1023, 2), (1024, 2), (2099, 2), (2100, 1)]);
    }

    #[test]
    fn a_slot_freed_below_the_lowest_searched_is_found_again() {
        let mut table = filled(3000);
        assert!(table.free(2500, one()));
        assert_eq!(table.reserve().map(|(s, g)| (s, g.get())), Some((2500, 2)));
        assert_eq!(table.reserve().map(|(s, g)| (s, g.get())), Some((3000, 1)));
        assert!(table.free(10, one()));
        assert!(table.free(2000, one()));
        assert_eq!(table.reserve().map(|(s, _)| s), Some(10));
        assert_eq!(table.reserve().map(|(s, _)| s), Some(2000));
        assert_eq!(table.reserve().map(|(s, _)| s), Some(3001));
    }

    #[test]
    fn a_stale_generation_is_not_live_and_cannot_be_written_or_freed() {
        let mut table = filled(3);
        assert!(table.free(1, one()));
        let (slot, generation) = table.reserve().unwrap();
        assert_eq!((slot, generation.get()), (1, 2));
        assert_eq!(table.live(1, one()), None, "reserved, and the old generation");
        assert!(!table.put(1, one(), 99));
        assert!(!table.free(1, one()));
        assert!(table.put(1, generation, 7));
        assert_eq!((table.live(1, generation), table.live(1, one())), (Some(&7), None));
        assert_eq!(table.live_generation(1), Some(2));
        assert!(table.free(1, generation));
        assert!(!table.free(1, generation), "already free");
        assert!(!table.put(1, generation, 8), "a freed slot is not written");
        assert_eq!((table.live_generation(1), table.live_generation(9)), (None, None));
    }

    #[test]
    fn a_reserved_slot_is_neither_live_nor_listed_until_written() {
        let mut table: Table<u32> = Table::new();
        let (slot, generation) = table.reserve().unwrap();
        assert_eq!((table.live(slot, generation), table.live_generation(slot)), (None, None));
        assert_eq!(table.records().count(), 0);
        assert!(matches!(table.slot(slot).map(Slot::state), Some(State::Reserved)));
        assert!(table.free(slot, generation), "a reserved slot can be freed unwritten");
        assert_eq!(table.reserve().map(|(s, g)| (s, g.get())), Some((0, 2)));
    }

    #[test]
    fn a_slot_whose_generation_can_grow_no_further_is_retired() {
        let mut table = filled(3);
        table.set_generation(1, u32::MAX);
        let last = NonZeroU32::MAX;
        assert!(table.free(1, last));
        assert_eq!(table.reserve().map(|(s, g)| (s, g.get())), Some((3, 1)), "slot 1 is never given again");
        table.set_generation(0, u32::MAX - 1);
        assert!(table.free(0, NonZeroU32::new(u32::MAX - 1).unwrap()));
        assert_eq!(table.reserve().map(|(s, g)| (s, g.get())), Some((0, u32::MAX)), "the last generation is given");
        // A retired slot before a free one is passed over, not given and found full.
        let mut table = filled(3);
        table.set_generation(1, u32::MAX);
        assert!(table.free(1, last) && table.free(2, NonZeroU32::MIN));
        assert_eq!(table.reserve().map(|(s, g)| (s, g.get())), Some((2, 2)));
    }

    #[test]
    fn a_full_table_gives_nothing_more() {
        let mut table: Table<u32> = Table::with_most(2);
        assert_eq!(table.reserve().map(|(s, _)| s), Some(0));
        assert_eq!(table.reserve().map(|(s, _)| s), Some(1));
        assert_eq!(table.reserve(), None);
        assert!(table.free(0, one()));
        assert_eq!(table.reserve().map(|(s, g)| (s, g.get())), Some((0, 2)), "a freed slot is still reused");
    }

    #[test]
    fn a_clone_shares_every_chunk_and_a_write_copies_only_its_own() {
        let table = filled(3000);
        let mut copy = table.clone();
        assert_eq!(copy.shared_chunks(&table), 3);
        assert!(copy.put(1500, one(), 0));
        assert_eq!(copy.shared_chunks(&table), 2);
        assert_eq!((table.live(1500, one()), copy.live(1500, one())), (Some(&1500), Some(&0)));
        assert!(copy.free(10, one()));
        assert_eq!(copy.shared_chunks(&table), 1);
        assert_eq!(table.live(10, one()), Some(&10), "the original is unchanged");
        assert!(table.slot(u32::MAX).is_none());
    }
}
