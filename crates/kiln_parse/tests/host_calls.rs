//! WP-1990: `__builtin_kiln_host_call` through the library interface a
//! tooling client uses (spec/host-calls.md §3): a Rust handler registered
//! with `parse_preprocessed_with_host`, the calls it receives and the
//! diagnostics of the translation unit. The handler logs every call, so
//! each test checks what reached it, not only what the program saw.
#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};

use kiln_diag::{DiagEngine, DiagOptions, Diagnostic, MemorySink};
use kiln_parse::{
    ConstexprLimits, HostCalls, HostError, HostOut, ParseOptions, parse_preprocessed_with_host,
};
use kiln_pp::{ClangFeatures, ModuleOptions, PpOptions, Preprocessor};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// One call the handler received: its op, integers and bytes.
type Call = (u32, Vec<u64>, Vec<u8>);

/// What the test handler saw and how it behaves.
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    counter: u64,
    cancelled: bool,
    /// Integers op 7 writes, whatever the capacity.
    overfill: usize,
}

/// The handler: op 1 a counter, op 2 an echo returning
/// `(ints << 32) | bytes`, op 3 fails, op 4 the capacities, op 5 arms
/// the cancellation op 6 fails on, op 7 writes `overfill` integers.
struct Handler(Arc<Mutex<State>>);

impl HostCalls for Handler {
    fn call(
        &mut self,
        op: u32,
        ints: &[u64],
        bytes: &[u8],
        out: &mut HostOut<'_>,
    ) -> Result<u64, HostError> {
        let mut s = self.0.lock().unwrap();
        s.calls.push((op, ints.to_vec(), bytes.to_vec()));
        match op {
            1 => {
                s.counter += 1;
                Ok(s.counter - 1)
            }
            2 => {
                out.write_ints(ints);
                out.write_bytes(bytes);
                Ok((ints.len() as u64) << 32 | bytes.len() as u64)
            }
            3 => Err(HostError::new("boom")),
            4 => Ok((out.int_capacity() as u64) << 32 | out.byte_capacity() as u64),
            5 => {
                s.cancelled = true;
                Ok(0)
            }
            6 if s.cancelled => Err(HostError::new("cancelled")),
            6 => Ok(0),
            7 => {
                let n = s.overfill;
                out.write_ints(&vec![9; n]);
                Ok(0)
            }
            _ => Err(HostError::new(format!("unknown op {op}"))),
        }
    }
}

/// The wrapper every test source starts with.
const PRE: &str = "using u64 = unsigned long long;\n\
constexpr u64 hc(unsigned op, const u64* in = nullptr, unsigned n_in = 0, \
const char* bin = nullptr, unsigned n_bin = 0, u64* out = nullptr, unsigned co = 0, \
char* bout = nullptr, unsigned cb = 0) {\n\
return __builtin_kiln_host_call(op, in, n_in, bin, n_bin, out, co, bout, cb);\n\
}\n";

/// The outcome of one translation unit.
struct Run {
    /// Each diagnostic and note as `L:C message` (the wrapper's lines
    /// first).
    diags: Vec<String>,
    /// The calls the handler received.
    calls: Vec<Call>,
    /// `Oven::host_call_count` at the end.
    count: u64,
    /// The handler came back in `ParsedTu::oven`.
    returned: bool,
}

impl Run {
    #[track_caller]
    fn clean(&self) {
        assert!(self.diags.is_empty(), "unexpected diagnostics:\n{}", self.diags.join("\n"));
    }

    #[track_caller]
    fn diag(&self, needle: &str) {
        assert!(
            self.diags.iter().any(|d| d.contains(needle)),
            "no diagnostic containing {needle:?}; got:\n{}",
            self.diags.join("\n")
        );
    }

    fn ops(&self) -> Vec<u32> {
        self.calls.iter().map(|c| c.0).collect()
    }
}

fn render(sm: &SourceManager, d: &Diagnostic, out: &mut Vec<String>) {
    let pos = sm
        .line_col(d.span.lo)
        .map_or_else(|| "-".to_owned(), |lc| format!("{}:{}", lc.line, lc.col));
    out.push(format!("{pos} {}", d.message));
    for n in &d.notes {
        render(sm, n, out);
    }
}

/// Parses `PRE` + `src` with the handler registered (`host`) or not.
fn run_with(src: &str, host: bool, state: State, limits: ConstexprLimits) -> Run {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", format!("{PRE}{src}")).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let opts = ParseOptions::default();
    let dopts = DiagOptions { clang_compat: opts.clang_compat, ..DiagOptions::default() };
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let mut ppo = PpOptions::new(opts.std);
    ppo.parser_pragmas = true;
    ppo.modules = Some(ModuleOptions::default());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, ppo);
    let features = ClangFeatures::new("x86_64-unknown-linux-gnu", opts.std, &[])
        .expect("x86-64 features")
        .with_kiln_host_calls(host);
    pp.set_feature_queries(Box::new(features));
    pp.enter_main_file(file);
    let state = Arc::new(Mutex::new(state));
    let handler = host.then(|| Box::new(Handler(Arc::clone(&state))) as Box<dyn HostCalls>);
    let target = Target::for_arch(Arch::X86_64);
    let (mut tu, _pp) = parse_preprocessed_with_host(
        pp,
        Arc::clone(&interner),
        &engine,
        target,
        &opts,
        limits,
        handler,
    );
    let count = tu.oven.host_call_count();
    let returned = tu.oven.take_host_calls().is_some();
    drop(tu);
    let mut diags = Vec::new();
    for d in sink.take() {
        render(&sm, &d, &mut diags);
    }
    let calls = std::mem::take(&mut state.lock().unwrap().calls);
    Run { diags, calls, count, returned }
}

fn run(src: &str) -> Run {
    run_with(src, true, State::default(), ConstexprLimits::default())
}

#[test]
fn absent_without_a_handler() {
    let r = run_with(
        "static_assert(!__has_builtin(__builtin_kiln_host_call));\n\
         static_assert(!__has_constexpr_builtin(__builtin_kiln_host_call));\n",
        false,
        State::default(),
        ConstexprLimits::default(),
    );
    // The wrapper's own call is the only error: an undeclared name.
    assert_eq!(
        r.diags,
        ["3:8 use of undeclared identifier '__builtin_kiln_host_call'"],
        "{:?}",
        r.diags
    );
    assert!(r.calls.is_empty() && !r.returned);
}

#[test]
fn present_with_a_handler() {
    let r = run("static_assert(__has_builtin(__builtin_kiln_host_call));\n\
         static_assert(__has_constexpr_builtin(__builtin_kiln_host_call));\n");
    r.clean();
    assert!(r.calls.is_empty() && r.returned);
}

#[test]
fn each_call_runs_once_in_order_and_is_never_memoized() {
    let r = run("consteval u64 three() { u64 a = hc(1); u64 b = hc(1); u64 c = hc(1); \
                 return a * 100 + b * 10 + c; }\n\
         static_assert(three() == 12);\n\
         constexpr u64 next() { return hc(1); }\n\
         consteval u64 one() { return next(); }\n\
         consteval u64 two() { return next(); }\n\
         static_assert(next() == 3);\n\
         static_assert(next() == 4);\n\
         static_assert(one() == 5);\n\
         static_assert(two() == 6);\n\
         static_assert(one() == 7);\n\
         consteval u64 twice() { return next() * 10 + next(); }\n\
         static_assert(twice() == 89);\n");
    r.clean();
    assert_eq!(r.ops(), [1; 10]);
    assert_eq!(r.count, 10);
}

#[test]
fn inputs_arrive_exactly() {
    let r = run("consteval u64 f() {\n\
           u64 v[5] = {10, 20, 30, 40, 0xffffffffffffffffull};\n\
           const char s[] = \"a\\0b\";\n\
           return hc(2, v + 2, 3, s, 3);\n\
         }\n\
         constexpr u64 r = f();\n");
    // The echo wrote 3 integers and 3 bytes into capacities of 0.
    r.diag("host call handler wrote 3 integers into a capacity of 0");
    assert_eq!(r.calls, [(2, vec![30, 40, u64::MAX], b"a\0b".to_vec())]);
}

#[test]
fn outputs_are_exactly_what_was_written() {
    let r = run("consteval bool f() {\n\
           u64 in[2] = {7, 8}; u64 o[2]; char b[4]{'w', 'x', 'y', 'z'};\n\
           if (hc(2, in, 2, \"ab\", 2, o, 2, b, 4) != ((2ull << 32) | 2)) return false;\n\
           return o[0] == 7 && o[1] == 8 && b[0] == 'a' && b[1] == 'b' && b[2] == 'y';\n\
         }\n\
         static_assert(f());\n\
         consteval u64 g() { u64 in[1] = {5}; u64 o[2]; hc(2, in, 1, nullptr, 0, o, 2); \
                             return o[1]; }\n\
         constexpr u64 x = g();\n");
    r.diag("read of uninitialized object is not allowed in a constant expression");
    assert_eq!(r.ops(), [2, 2]);
}

#[test]
fn overlapping_output_reads_inputs_first() {
    let r = run("consteval bool f() {\n\
           u64 buf[4] = {1, 2, 3, 0};\n\
           hc(2, buf, 3, nullptr, 0, buf + 1, 3);\n\
           return buf[0] == 1 && buf[1] == 1 && buf[2] == 2 && buf[3] == 3;\n\
         }\n\
         static_assert(f());\n\
         consteval u64 caps() { u64 o[3]; char b[5]; return hc(4, nullptr, 0, nullptr, 0, o, 3, b, 5); }\n\
         static_assert(caps() == ((3ull << 32) | 5));\n");
    r.clean();
}

#[test]
fn failed_checks_do_not_call_the_handler() {
    let r = run("consteval u64 a() { u64 in[1] = {1}; return hc(2, in, 2); }\n\
         constexpr u64 x1 = a();\n\
         consteval u64 b() { const u64 c[1] = {0}; return hc(4, nullptr, 0, nullptr, 0, \
                             const_cast<u64*>(c), 1); }\n\
         constexpr u64 x2 = b();\n\
         consteval u64 c() { return hc(4, nullptr, 0, nullptr, 0, nullptr, 0, \
                             const_cast<char*>(\"ab\"), 2); }\n\
         constexpr u64 x3 = c();\n\
         u64 glob[2];\n\
         consteval u64 d() { return hc(4, nullptr, 0, nullptr, 0, glob, 2); }\n\
         constexpr u64 x4 = d();\n\
         consteval u64 e() { return hc(2, nullptr, 1); }\n\
         constexpr u64 x5 = e();\n\
         consteval u64 f() { u64 in[2]; in[0] = 1; return hc(2, in, 2); }\n\
         constexpr u64 x6 = f();\n");
    r.diag("'in' of __builtin_kiln_host_call does not point to 2 consecutive elements");
    r.diag("'out' of __builtin_kiln_host_call designates a const object");
    r.diag("'bout' of __builtin_kiln_host_call designates a string literal");
    r.diag(
        "'out' of __builtin_kiln_host_call designates an object whose lifetime began outside \
         the evaluation",
    );
    r.diag("'in' of __builtin_kiln_host_call is null with a length of 1");
    r.diag("'in' of __builtin_kiln_host_call element 1 is uninitialized");
    assert!(r.calls.is_empty(), "{:?}", r.calls);
}

#[test]
fn an_error_ends_the_evaluation_uncatchably() {
    let r = run("consteval u64 f() { try { return hc(3); } catch (...) { return 1; } }\n\
         constexpr u64 x = f();\n\
         constinit u64 y = f();\n");
    r.diag("host call failed: boom");
    // Once for each initializer: the deferred report of the immediate
    // invocation reuses its failure.
    assert_eq!(r.ops(), [3, 3]);
}

#[test]
fn writing_past_a_capacity_ends_the_evaluation() {
    let r = run_with(
        "consteval u64 f() { u64 o[2]; return hc(7, nullptr, 0, nullptr, 0, o, 2); }\n\
         constexpr u64 x = f();\n",
        true,
        State { overfill: 3, ..State::default() },
        ConstexprLimits::default(),
    );
    r.diag("host call handler wrote 3 integers into a capacity of 2");
}

#[test]
fn cancellation_deep_in_the_stack_runs_no_destructor() {
    let r = run("struct D { constexpr ~D() { hc(1); } };\n\
         consteval u64 deep(int n) { D d; if (n == 0) { hc(5); return hc(6); } \
                                     return deep(n - 1); }\n\
         constexpr u64 r = deep(300);\n");
    r.diag("host call failed: cancelled");
    assert_eq!(r.ops(), [5, 6]);
}

#[test]
fn a_definition_calls_nothing_and_an_evaluation_calls_once() {
    let r = run("consteval u64 f() { const u64 n = hc(1); return n + n; }\n\
         constexpr u64 g() { const u64 n = hc(1); return n * 3; }\n");
    r.clean();
    assert!(r.calls.is_empty(), "{:?}", r.calls);
    let r = run("consteval u64 f() { const u64 n = hc(1); return n + n + 1; }\n\
         static_assert(f() == 1);\n");
    r.clean();
    assert_eq!(r.ops(), [1]);
}

#[test]
fn speculative_evaluations_are_an_error() {
    let r = run("consteval u64 f() { const u64 n = hc(1) + 1; int a[n]; a[0] = 1; return 0; }\n");
    r.diag("host call during speculative evaluation");
    let r = run("const u64 g = hc(1);\n");
    r.diag("host call during speculative evaluation");
    assert!(r.calls.is_empty(), "{:?}", r.calls);
}

#[test]
fn unevaluated_operands_call_nothing() {
    let r = run("static_assert(!noexcept(hc(1)));\n\
         static_assert(sizeof(hc(1)) == 8);\n\
         decltype(hc(1)) x = 0;\n\
         template <class T> concept C = requires { hc(1); };\n\
         static_assert(C<int>);\n");
    r.clean();
    assert!(r.calls.is_empty(), "{:?}", r.calls);
}

#[test]
fn the_step_budget_counts_the_elements() {
    let mut limits = ConstexprLimits::default();
    limits.budget.steps = 60;
    // 1 + 100 steps for the call: over the budget before the handler.
    let src = format!(
        "consteval u64 f() {{ return hc(2, nullptr, 0, \"{}\", 100); }}\n\
         constexpr u64 x = f();\n",
        "x".repeat(100)
    );
    let r = run_with(&src, true, State::default(), limits);
    r.diag("constexpr evaluation hit maximum step limit");
    assert!(r.calls.is_empty(), "{:?}", r.calls);
}
