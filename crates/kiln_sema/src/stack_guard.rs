//! The native-stack guard (spec/sema-templates.md P4 (c), §10.3, D1, Q4).
//!
//! The front end (parser, sema and the Oven, which the parser reaches
//! through sema) runs on one thread whose stack size the driver chooses
//! (`-fkiln-stack-size`, default 1 GiB). Template instantiation, the
//! parser's nesting and the substituter recurse natively, so a program can
//! ask for more stack than there is. [`StackGuard`] turns that into the
//! fatal diagnostic "kiln: stack exhausted during …" instead of a crash.
//!
//! The estimate is safe Rust: [`StackGuard::install`] records the address
//! of a local at thread start, and a check compares it with the address
//! of a local in the checking frame. The distance is the stack in use;
//! less than [`RED_ZONE`] left trips the guard. On a thread with no guard
//! installed (unit tests, library callers) every check passes.

use std::cell::Cell;

use kiln_diag::{Diag, DiagId};
use kiln_source::{Loc, Span};

use crate::Sema;

/// The stack that must remain when a check passes: the deepest recursion
/// between two checks, plus the diagnostic and unwinding paths, must fit
/// in it. Measured: one template-instantiation level is under 300 KiB in
/// a debug build; one parser nesting level a few KiB.
pub const RED_ZONE: usize = 1 << 20;

/// The default stack of the compiling thread (Q4, decided in WP-086): 1
/// GiB of reserved address space, touched only as deep as the program
/// recurses. 256 MiB, the spec's first figure, does not reach the default
/// `-ftemplate-depth=1024` in a debug build.
pub const DEFAULT_STACK_SIZE: usize = 1 << 30;

/// The smallest `-fkiln-stack-size` honored: the red zone plus room for
/// an ordinary translation unit. Smaller requests are raised to it.
pub const MIN_STACK_SIZE: usize = 4 << 20;

/// What was running when the stack ran out (the diagnostic's selector).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackPhase {
    /// The parser's nesting (expressions, declarators, braces,
    /// template-argument lists, statements).
    Parsing = 0,
    /// Template instantiation and substitution.
    Instantiation = 1,
    /// Constant evaluation entered from sema.
    ConstantEvaluation = 2,
}

/// A thread's stack: where it starts and how big it is (size 0: no guard,
/// every check passes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackGuard {
    base: usize,
    size: usize,
}

thread_local! {
    static CURRENT: Cell<StackGuard> = const { Cell::new(StackGuard::DISABLED) };
}

/// The address of a local of the calling frame (safe: a reference turned
/// into its address, never dereferenced).
#[inline]
fn stack_address() -> usize {
    let probe = 0u8;
    std::ptr::from_ref(std::hint::black_box(&probe)).addr()
}

impl Default for StackGuard {
    /// The guard installed on this thread ([`StackGuard::current`]).
    fn default() -> Self {
        StackGuard::current()
    }
}

impl StackGuard {
    /// No guard: every check passes.
    pub const DISABLED: StackGuard = StackGuard { base: 0, size: 0 };

    /// A guard for a stack of `size` bytes starting at address `base`
    /// (for tests of the arithmetic).
    pub const fn new(base: usize, size: usize) -> StackGuard {
        StackGuard { base, size }
    }

    /// Installs the guard of the calling thread, whose stack is `size`
    /// bytes: call it first thing on that thread (the driver's compile
    /// thread). Returns the guard.
    #[inline(never)]
    pub fn install(size: usize) -> StackGuard {
        let g = StackGuard { base: stack_address(), size };
        CURRENT.with(|c| c.set(g));
        g
    }

    /// The guard installed on the calling thread, or
    /// [`StackGuard::DISABLED`].
    pub fn current() -> StackGuard {
        CURRENT.with(Cell::get)
    }

    /// Whether the guard checks anything.
    pub const fn is_enabled(&self) -> bool {
        self.size != 0
    }

    /// The stack left when a frame's locals are at `here` (`None` without
    /// a guard). The stack may grow either way; whatever is beyond
    /// `size` from the base counts as nothing left.
    pub const fn remaining_at(&self, here: usize) -> Option<usize> {
        if self.size == 0 {
            return None;
        }
        Some(self.size.saturating_sub(self.base.abs_diff(here)))
    }

    /// The stack left in the calling frame (`None` without a guard).
    #[inline]
    pub fn remaining(&self) -> Option<usize> {
        self.remaining_at(stack_address())
    }

    /// Whether less than [`RED_ZONE`] is left at `here`.
    pub const fn exhausted_at(&self, here: usize) -> bool {
        match self.remaining_at(here) {
            Some(r) => r < RED_ZONE,
            None => false,
        }
    }

    /// Whether less than [`RED_ZONE`] is left in the calling frame.
    #[inline]
    pub fn exhausted(&self) -> bool {
        self.size != 0 && self.exhausted_at(stack_address())
    }
}

/// Parses a `-fkiln-stack-size=` value: bytes, or a number with a `K`,
/// `M` or `G` suffix (binary units). `None` if malformed or zero; values
/// below [`MIN_STACK_SIZE`] are raised to it.
pub fn parse_stack_size(v: &str) -> Option<usize> {
    let (digits, shift) = match v.as_bytes().last()? {
        b'k' | b'K' => (&v[..v.len() - 1], 10),
        b'm' | b'M' => (&v[..v.len() - 1], 20),
        b'g' | b'G' => (&v[..v.len() - 1], 30),
        _ => (v, 0),
    };
    let n: usize = digits.parse().ok()?;
    if n == 0 {
        return None;
    }
    let bytes = n.checked_mul(1usize << shift)?;
    Some(bytes.max(MIN_STACK_SIZE))
}

impl Sema<'_, '_> {
    /// Whether the stack is nearly exhausted (a cheap check for the
    /// recursive entries; spec/sema-templates.md P4 (c)).
    #[inline]
    pub fn stack_low(&self) -> bool {
        self.inst.guard.exhausted()
    }

    /// The guard's check at a recursive entry: false when the stack is
    /// nearly exhausted, after emitting the fatal "kiln: stack exhausted
    /// during …" at `loc` with the instantiation stack (once per
    /// translation unit). Every later check then fails too, so the
    /// recursion unwinds.
    #[inline]
    pub fn stack_ok(&self, phase: StackPhase, loc: Loc) -> bool {
        if self.inst.stack_exhausted.get() {
            return false;
        }
        if !self.stack_low() {
            return true;
        }
        self.report_stack_exhausted(phase, loc);
        false
    }

    /// Whether the guard tripped in this translation unit.
    pub fn stack_exhausted(&self) -> bool {
        self.inst.stack_exhausted.get()
    }

    #[cold]
    #[inline(never)]
    fn report_stack_exhausted(&self, phase: StackPhase, loc: Loc) {
        if self.inst.stack_exhausted.replace(true) {
            return;
        }
        let mut d = Diag::new(DiagId::sema_stack_exhausted, Span::empty(loc)).arg(phase as u32);
        // Not a SFINAE error: the notes print whatever the context.
        if !self.inst.stack.is_empty() {
            d = d.context(self.inst.stack.context_key());
            d.prepend_notes(self.inst_context_notes());
        }
        d.emit(self.diags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P4 (c): the remaining stack is the size less the distance from the
    /// base, in either direction, never negative.
    #[test]
    fn remaining_arithmetic() {
        let g = StackGuard::new(0x1000_0000, 8 << 20);
        assert_eq!(g.remaining_at(0x1000_0000), Some(8 << 20));
        assert_eq!(g.remaining_at(0x1000_0000 - (3 << 20)), Some(5 << 20));
        assert_eq!(g.remaining_at(0x1000_0000 + (3 << 20)), Some(5 << 20));
        assert_eq!(g.remaining_at(0x1000_0000 - (9 << 20)), Some(0));
        assert!(!g.exhausted_at(0x1000_0000 - (7 << 20)));
        assert!(g.exhausted_at(0x1000_0000 - (7 << 20) - 1));
        assert!(g.exhausted_at(0));
    }

    /// Without a guard nothing is ever exhausted.
    #[test]
    fn disabled_guard_passes() {
        let g = StackGuard::DISABLED;
        assert!(!g.is_enabled());
        assert_eq!(g.remaining_at(12345), None);
        assert!(!g.exhausted_at(0));
        assert!(!g.exhausted());
        // A fresh test thread has none installed.
        assert_eq!(std::thread::spawn(StackGuard::current).join().ok(), Some(StackGuard::DISABLED));
    }

    /// An installed guard sees the stack it records, and recursion
    /// measurably consumes it.
    #[test]
    fn installed_guard_tracks_recursion() {
        #[inline(never)]
        fn deepest(g: StackGuard, n: u32) -> usize {
            let pad = std::hint::black_box([0u8; 512]);
            let r = if n == 0 { g.remaining().unwrap_or(0) } else { deepest(g, n - 1) };
            r + usize::from(pad[0])
        }
        let size = 16 << 20;
        let h = std::thread::Builder::new()
            .stack_size(size)
            .spawn(move || {
                let g = StackGuard::install(size);
                assert_eq!(StackGuard::current(), g);
                let top = g.remaining().unwrap_or(0);
                let deep = deepest(g, 1000);
                (top, deep, g.exhausted())
            })
            .expect("spawn");
        let (top, deep, exhausted) = h.join().expect("join");
        assert!(top > size - (64 << 10), "{top}");
        assert!(deep + 1000 * 512 <= top, "{deep} {top}");
        assert!(!exhausted);
    }

    #[test]
    fn stack_size_values() {
        assert_eq!(parse_stack_size("64M"), Some(64 << 20));
        assert_eq!(parse_stack_size("2g"), Some(2 << 30));
        assert_eq!(parse_stack_size("8192k"), Some(8 << 20));
        assert_eq!(parse_stack_size("1"), Some(MIN_STACK_SIZE));
        assert_eq!(parse_stack_size("0"), None);
        assert_eq!(parse_stack_size("M"), None);
        assert_eq!(parse_stack_size("12x"), None);
        assert_eq!(parse_stack_size(""), None);
        assert_eq!(parse_stack_size("99999999999999999999G"), None);
    }
}
