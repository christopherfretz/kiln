//! The "current activity" breadcrumb for internal-compiler-error reports
//! (spec/driver.md "Internal compiler errors").
//!
//! Each thread carries one [`Cell`] naming the pipeline [`Phase`] it is in
//! and the source location it last reached. The pipeline updates it at a
//! few coarse points (entering a phase, each top-level declaration, each
//! constant evaluation); the driver's panic hook reads it, so an ICE can
//! say where it happened. An update is a thread-local store of eight
//! bytes: nothing is allocated and nothing is formatted until a crash.
//!
//! The cell also holds the hidden `-fkiln-crash-for-testing=<phase>`
//! request: entering that phase (or, for [`Phase::Parse`], reaching the
//! first declaration) panics on purpose, so tests can exercise the crash
//! path. The request is per thread, so in-process tests that run in
//! parallel never see each other's.
use crate::Loc;
use std::cell::Cell;

/// A pipeline phase, as an ICE report names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    /// The driver itself (argument handling, toolchain setup, linking).
    Driver,
    /// Lexing and preprocessing.
    Preprocess,
    /// Parsing (with the semantic analysis interleaved with it).
    Parse,
    /// End-of-translation-unit semantic analysis (pending instantiations,
    /// final checks).
    Sema,
    /// Constant evaluation.
    Oven,
    /// LLVM IR generation.
    Codegen,
}

impl Phase {
    /// The phase's name in reports and in `-fkiln-crash-for-testing=`.
    pub const fn name(self) -> &'static str {
        match self {
            Phase::Driver => "driver",
            Phase::Preprocess => "preprocess",
            Phase::Parse => "parse",
            Phase::Sema => "sema",
            Phase::Oven => "oven",
            Phase::Codegen => "codegen",
        }
    }

    /// The phase named `s` ([`Phase::name`], or `lex` / `pp` for
    /// [`Phase::Preprocess`]).
    pub fn parse(s: &str) -> Option<Phase> {
        Some(match s {
            "driver" => Phase::Driver,
            "lex" | "pp" | "preprocess" => Phase::Preprocess,
            "parse" => Phase::Parse,
            "sema" => Phase::Sema,
            "oven" => Phase::Oven,
            "codegen" => Phase::Codegen,
            _ => return None,
        })
    }
}

/// What a thread is doing: the phase and the last location reached in it
/// ([`Loc::INVALID`] when none is known).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activity {
    /// The phase.
    pub phase: Phase,
    /// The last source location reached.
    pub loc: Loc,
}

#[derive(Clone, Copy)]
struct State {
    phase: Phase,
    loc: Loc,
    crash: Option<Phase>,
}

thread_local! {
    static STATE: Cell<State> =
        const { Cell::new(State { phase: Phase::Driver, loc: Loc::INVALID, crash: None }) };
}

/// The calling thread's current activity.
pub fn current() -> Activity {
    let s = STATE.with(Cell::get);
    Activity { phase: s.phase, loc: s.loc }
}

/// Records that the calling thread reached `loc` in its current phase.
#[inline]
pub fn at(loc: Loc) {
    STATE.with(|c| {
        let mut s = c.get();
        s.loc = loc;
        c.set(s);
        if s.crash == Some(s.phase) {
            crash_for_testing(s.phase);
        }
    });
}

/// Enters `phase` at `loc` until the returned guard drops, which restores
/// the previous activity.
#[inline]
#[must_use = "the phase ends when the guard drops"]
pub fn enter(phase: Phase, loc: Loc) -> PhaseGuard {
    STATE.with(|c| {
        let s = c.get();
        c.set(State { phase, loc, crash: s.crash });
        let guard = PhaseGuard { phase: s.phase, loc: s.loc };
        // The parser checks per declaration ([`at`]), so the report
        // carries the declaration's location.
        if s.crash == Some(phase) && phase != Phase::Parse {
            crash_for_testing(phase);
        }
        guard
    })
}

/// Restores the activity [`enter`] replaced.
#[derive(Debug)]
pub struct PhaseGuard {
    phase: Phase,
    loc: Loc,
}

impl Drop for PhaseGuard {
    #[inline]
    fn drop(&mut self) {
        STATE.with(|c| {
            let s = c.get();
            c.set(State { phase: self.phase, loc: self.loc, crash: s.crash });
        });
    }
}

/// Sets (or, with `None`, clears) the calling thread's
/// `-fkiln-crash-for-testing=` request.
pub fn set_crash_for_testing(phase: Option<Phase>) {
    STATE.with(|c| {
        let mut s = c.get();
        s.crash = phase;
        c.set(s);
    });
}

#[cold]
#[inline(never)]
fn crash_for_testing(phase: Phase) -> ! {
    panic!("crash requested by -fkiln-crash-for-testing={}", phase.name())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_restores_on_drop() {
        assert_eq!(current().phase, Phase::Driver);
        {
            let _p = enter(Phase::Parse, Loc::INVALID);
            at(Loc::from_raw(7));
            assert_eq!(current(), Activity { phase: Phase::Parse, loc: Loc::from_raw(7) });
            {
                let _o = enter(Phase::Oven, Loc::from_raw(9));
                assert_eq!(current().phase, Phase::Oven);
            }
            assert_eq!(current(), Activity { phase: Phase::Parse, loc: Loc::from_raw(7) });
        }
        assert_eq!(current(), Activity { phase: Phase::Driver, loc: Loc::INVALID });
    }

    #[test]
    fn names_round_trip() {
        for p in [
            Phase::Driver,
            Phase::Preprocess,
            Phase::Parse,
            Phase::Sema,
            Phase::Oven,
            Phase::Codegen,
        ] {
            assert_eq!(Phase::parse(p.name()), Some(p));
        }
        assert_eq!(Phase::parse("lex"), Some(Phase::Preprocess));
        assert_eq!(Phase::parse("link"), None);
    }

    #[test]
    fn crash_request_fires_only_in_its_phase() {
        set_crash_for_testing(Some(Phase::Oven));
        {
            let _p = enter(Phase::Parse, Loc::INVALID);
            at(Loc::from_raw(3));
        }
        let r = std::panic::catch_unwind(|| {
            let _o = enter(Phase::Oven, Loc::INVALID);
        });
        set_crash_for_testing(None);
        assert!(r.is_err());
        // Unwinding dropped the panicking `enter`'s guard.
        assert_eq!(current().phase, Phase::Driver);
    }

    #[test]
    fn parse_request_fires_at_a_declaration() {
        set_crash_for_testing(Some(Phase::Parse));
        let entered = std::panic::catch_unwind(|| {
            let _p = enter(Phase::Parse, Loc::INVALID);
        });
        assert!(entered.is_ok());
        let reached = std::panic::catch_unwind(|| {
            let _p = enter(Phase::Parse, Loc::INVALID);
            at(Loc::from_raw(5));
        });
        set_crash_for_testing(None);
        assert!(reached.is_err());
    }
}
