//! Opt-in preprocessor counters (`--features pp-profile`), printed to
//! stderr at the end of [`crate::print_preprocessed`] when the environment
//! variable `KILN_PP_PROFILE=1` is set. Without the feature every hook
//! compiles to nothing. Used by WP-056 to find super-linear paths without
//! a sampling profiler (the sandbox forbids `perf_event_open`).

#[cfg(feature = "pp-profile")]
mod imp {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    macro_rules! counters {
        ($($name:ident),* $(,)?) => {
            #[derive(Clone, Copy)]
            #[allow(non_camel_case_types, dead_code)]
            pub(crate) enum C { $($name),* }
            const NAMES: &[&str] = &[$(stringify!($name)),*];
        };
    }
    counters!(
        loc_diff,
        loc_diff_slow,
        loc_diff_steps,
        entry_of,
        exp_macro,
        exp_macro_arg,
        relocate_runs,
        macro_lookup,
        spelling_slow,
        presumed,
        buf_new,
        buf_dropped,
        tokens_out,
        files_entered,
        file_reentries,
        raw_tokens,
        skip_tokens,
        if_evals,
        directives,
        t_has_include_ns,
        t_directive_ns,
        t_skip_ns,
        t_if_ns,
        t_include_ns,
        t_define_ns,
        t_relocate_ns,
        t_print_ns,
        t_total_ns,
    );

    static COUNTS: [AtomicU64; NAMES.len()] = [const { AtomicU64::new(0) }; NAMES.len()];

    pub(crate) fn add(c: C, n: u64) {
        COUNTS[c as usize].fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) struct Timer(C, Instant);

    impl Timer {
        pub(crate) fn new(c: C) -> Timer {
            Timer(c, Instant::now())
        }
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            add(self.0, u64::try_from(self.1.elapsed().as_nanos()).unwrap_or(u64::MAX));
        }
    }

    pub(crate) fn report() {
        if std::env::var_os("KILN_PP_PROFILE").is_none_or(|v| v != "1") {
            return;
        }
        for (name, c) in NAMES.iter().zip(&COUNTS) {
            let v = c.load(Ordering::Relaxed);
            if let Some(n) = name.strip_prefix("t_") {
                eprintln!("pp-profile {n:>16} {:>8}.{:03} ms", v / 1_000_000, v / 1000 % 1000);
            } else {
                eprintln!("pp-profile {name:>16} {v:>12}");
            }
        }
    }
}

#[cfg(feature = "pp-profile")]
pub(crate) use imp::{C, Timer, add, report};

/// Prints the counters (no-op without the `pp-profile` feature).
#[cfg(not(feature = "pp-profile"))]
pub(crate) fn report() {}

/// Adds `$n` (default 1) to counter `$c`.
macro_rules! count {
    ($c:ident) => {
        $crate::profile::count!($c, 1)
    };
    ($c:ident, $n:expr) => {{
        #[cfg(feature = "pp-profile")]
        $crate::profile::add($crate::profile::C::$c, ($n) as u64);
    }};
}

/// Times the rest of the enclosing block into counter `$c`.
macro_rules! time {
    ($c:ident) => {
        #[cfg(feature = "pp-profile")]
        let _prof_timer = $crate::profile::Timer::new($crate::profile::C::$c);
    };
}

pub(crate) use {count, time};
