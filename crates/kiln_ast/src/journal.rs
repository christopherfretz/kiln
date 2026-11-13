//! Opt-in change journals (WP-658).
//!
//! A side index sema keeps over AST data (the redeclaration index,
//! spec/sema.md §3) must hear about the few mutations that can change
//! what it indexed: a lookup-set slot replaced or removed, a declaration
//! retyped, a redeclaration group's latest declaration moved. Each such
//! mutator pushes onto a [`Journal`] once a reader has started it; the
//! reader drains it before it trusts the index. A journal holds at most
//! [`Journal::CAP`] events: past that it drops them and records an
//! overflow, after which the reader rebuilds instead (so an undrained
//! journal stays bounded).
#![warn(missing_docs)]

/// A bounded event log, off until [`Journal::start`].
#[derive(Debug, Clone, kiln_snap::Snap)]
pub struct Journal<T> {
    on: bool,
    overflowed: bool,
    items: Vec<T>,
}

impl<T> Default for Journal<T> {
    fn default() -> Self {
        Self { on: false, overflowed: false, items: Vec::new() }
    }
}

impl<T> Journal<T> {
    /// The most events kept between two drains.
    pub const CAP: usize = 1 << 16;

    /// Starts recording (idempotent).
    pub fn start(&mut self) {
        self.on = true;
    }

    /// Whether recording has started.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Records `t`, if recording.
    #[inline]
    pub fn push(&mut self, t: T) {
        if !self.on || self.overflowed {
            return;
        }
        if self.items.len() >= Self::CAP {
            self.overflowed = true;
            self.items = Vec::new();
            return;
        }
        self.items.push(t);
    }

    /// The memory row of the journal (`Ast::memory`'s `journal.*` rows,
    /// WP-661): the events held now, and their buffer.
    pub fn mem_row(&self, name: &str) -> crate::mem::MemRow {
        crate::mem::MemRow::of_vec(name, &self.items)
    }

    /// The events since the last drain, and whether some were dropped
    /// (then the reader must not trust anything it derived before).
    pub fn drain(&mut self) -> (Vec<T>, bool) {
        let overflowed = std::mem::replace(&mut self.overflowed, false);
        (std::mem::take(&mut self.items), overflowed)
    }
}

#[cfg(test)]
mod tests {
    use super::Journal;

    #[test]
    fn records_only_once_started_and_overflows_to_a_flag() {
        let mut j = Journal::<u32>::default();
        j.push(1);
        assert_eq!(j.drain(), (vec![], false));
        j.start();
        j.push(2);
        assert_eq!(j.drain(), (vec![2], false));
        for i in 0..=Journal::<u32>::CAP as u32 {
            j.push(i);
        }
        j.push(7);
        assert_eq!(j.drain(), (vec![], true));
        j.push(3);
        assert_eq!(j.drain(), (vec![3], false));
    }
}
