# Evaluation sessions (WP-1995)

Status: implemented (WP-1995). Code: `crates/kiln_sema/src/session.rs`
(the session, entries, runs, reports, diagnostic replay),
`crates/kiln_oven/src/session.rs` (budgets, cancellation, the Oven side
of a run), `crates/kiln_diag/src/engine.rs` (`DiagEngine::capture_begin`,
the per-run capture), `crates/kiln_parse/src/lib.rs`
(`parse_preprocessed_retained`), `crates/kiln_driver/src/session.rs`
(`with_eval_session`, from a command line). Standard: [expr.const]
(the run is an immediate invocation, [expr.const]/16, evaluated at the
end of the translation unit), [temp.point]/8 (the instantiations a run
needs), [except.terminate] (an exception leaving the entry).

A tooling client compiles one translation unit (a *transform*), then
evaluates a `consteval` *entry* function of it many times, once per
input. Each evaluation is a *run*. A run talks to the client through
host calls (spec/host-calls.md) and leaves nothing behind that a later
run can observe. Nothing here changes what kiln does for a translation
unit that opens no session.

## 1. Design: retain the finished state

The session keeps the finished front end of the translation unit: the
`Sema` after the end-of-translation-unit work (pending instantiations
done, end-of-unit checks run), which owns the AST and the Oven. A run
is an immediate invocation evaluated against that state, as one at the
end of the translation unit would be.

The alternative, re-entering from a parsed prefix (`parse_prefix`,
`continue_preprocessed`) per run, was rejected: every run would repeat
the main file's parse and the end-of-unit work (instantiating what the
transform uses), which is the cost a session exists to pay once; and a
prefix cannot hold the transform's own definitions, so each run would
have to re-declare them. Retaining costs the memory of the finished
front end, which the client holds anyway while it runs (§10 has the
measurements), and nothing per run beyond the run's own state.

## 2. The interface

In `kiln_sema::session` (re-exported by `kiln_parse`; the budget,
token and value types are `kiln_oven`'s):

```rust
// Opening.
EvalSession::open(sema: Sema<'a, 'sm>) -> Result<EvalSession<'a, 'sm>, OpenError>;
kiln_parse::parse_preprocessed_retained(pp, interner, diags, target, opts, limits, tu_host)
    -> (Sema<'a, 'sm>, Preprocessor<'sm, 'a>);
kiln_driver::session::with_eval_session(argv0, command_line, io, |front_end| ...) -> R;

// Entries.
session.entry(f: DeclId) -> Result<Entry, EntryError>;
session.entry_named("ns::name") -> Result<Entry, EntryError>;

// Runs.
session.run(entry, &mut dyn HostCalls, RunBudget, &CancelToken) -> RunReport;

// Memory.
session.kept() -> KeptMemory;
session.clear_memo() -> usize;

// Closing.
drop(session); session.into_sema() -> Sema;
```

- **Opening from parts.** `parse_preprocessed_retained` parses as
  `parse_preprocessed_with_host` does and returns the finished `Sema`
  instead of a `ParsedTu`; `EvalSession::open` takes it. The handler
  given to the parse (`tu_host`) is the translation unit's: it makes
  `__builtin_kiln_host_call` visible (spec/host-calls.md §1) and serves
  the evaluations of the translation unit itself. `open` takes it out of
  the Oven ([`EvalSession::take_tu_host_calls`] returns it): runs use
  only their own handler (§6).
- **Opening from a command line.** `with_eval_session` takes a driver
  command line naming exactly one C++ source input (the driver's
  include paths, `-D`/`-U`, `-std=`, `-resource-dir`, `-stdlib=`,
  `--target=`, `-W` options and constexpr limits apply as for
  `-fsyntax-only`) and calls the closure with a `SessionFrontEnd`
  (`#[non_exhaustive]`: the session, the source manager, the main file;
  later fields may be added) or with the reason no session was opened.
  The builtin is visible (a placeholder handler that fails every call is
  the translation unit's). The translation unit's own diagnostics go to
  `io`'s standard error as the driver prints them. The closure form is
  forced by the borrows: the session borrows the diagnostics engine,
  which borrows the source manager, both owned by the call.
- **Refusal.** `open` fails with `OpenError::TuHasErrors { errors }`
  when the translation unit reported an error (an error-limit fatal
  included); `with_eval_session` then also reports
  `drv_eval_session_tu_errors` ("cannot open an evaluation session: the
  translation unit has N error(s)") on standard error. A command line
  that does not name one C++ source input, or that the driver rejects,
  is `SessionRefused::CommandLine`.

## 3. Entries

An entry is a function the session may run:

- a **`consteval` function** (an immediate function by its declaration;
  a `constexpr` function, even one escalated to immediate, is
  `EntryError::NotConsteval`); a function template is not a function
  (`NotAFunction`), but a `consteval` specialization of one is accepted
  by `DeclId`;
- with **no parameters** (a C-style `(void)` included; not variadic):
  else `HasParameters`;
- returning **`void` or an integral type** ([basic.fundamental]/11:
  `bool`, the character types, the signed and unsigned integer types,
  `__int128`; cv-qualifiers ignored): else `WrongReturnType`. An
  enumeration is not integral, and `_BitInt(N)` is not accepted;
- **defined** in the translation unit: else `Undefined`.

`entry_named` looks up a qualified name (`f`, `ns::f`, `::ns::f`,
`ns::C::f` for a static member function; a scope may be named
through a namespace alias or a type alias of a class; inline
namespaces are searched) from the translation unit's scope ([namespace.qual],
[class.qual]): no declaration is `NotFound`, more than one function
(an overload set) is `Overloaded`, a non-function is `NotAFunction`;
the one function is then checked as above. An `Entry` is a small
`Copy` value; any number of entries per session may be resolved and
run, in any order.

Deferred (follow-up rows): parameters (`std::meta::info` or integers
given per run), class-typed results, `constexpr` entries.

## 4. A run

`run(entry, host, budget, cancel)` evaluates a call of the entry
without arguments as a manifestly constant-evaluated immediate
invocation ([expr.const]/16) at the end of the translation unit. It is
not plainly constant-evaluated ([expr.const.core]/2.32): it cannot
inject declarations (`std::meta::define_aggregate` fails), as for any
immediate invocation outside a consteval block. The result is checked
as any immediate invocation's: an allocation not deallocated by the end
of the run makes it fail. An error reported by semantic work the run
needed (an instantiation it caused, or one replayed, §7) fails the run
with `RunError::Failed` even when the evaluation completed: that error
is the run's diagnostic.

`RunReport`:

| field | meaning |
|---|---|
| `result` | `Ok(RunValue)`: `Void`, `Bool(b)`, `Signed(i128)`, `Unsigned(u128)` (by the return type's signedness); `Err(RunError)`, §8 |
| `steps` | steps the run executed (spec/oven.md "Limits"), §5 |
| `host_calls` | handler calls the run made |
| `peak_heap` | the run's peak live abstract memory (frame and heap bytes; spec/oven.md `peak_bytes`) |
| `calls` | function calls the run made |
| `diagnostics` | the run's diagnostics, §7, in order |

## 5. Isolation and what a session keeps

Every run starts from the post-translation-unit state. **Reset at the
end of every run** (nothing of it reaches a later run): the frame and
heap spaces and every object in them (transient allocations, the
`std::vector`s a run builds), the call stack, cleanups, exceptions in
flight or caught, the run's counters and impurity marks, the host-call
buffers, the diagnostics deduplication set and counts (§7) and the
state of any union or `mutable` member of a run's object. A run can
modify no object whose lifetime began outside it ([expr.const]/5.16),
so the static objects of the translation unit (the values of
`constexpr` variables) are never changed by a run.

**Kept** (deterministic caches: a run reads them, and gets the same
answer whether an earlier run or this one filled them):
- template instantiations and the other semantic work a run triggers
  (an implicit member defined on demand, a class completed, a
  reflection query answered): the run's point of instantiation is the
  end of the translation unit ([temp.point]/8), so its instantiations
  are those any later point would make. Their diagnostics are
  replayed (§7);
- compiled bytecode, the run's entry stub, resolved globals and the
  static objects that hold `constexpr` variables' values, interned
  string-literal objects;
- the memoization cache (spec/oven.md "Memoization"): bounded LRU of
  pure calls' scalar results. A hit charges exactly what the call
  cost (steps, calls, bytes, depth), so whether an earlier run warmed
  it changes no counter and no budget decision. `clear_memo()` drops
  every entry and returns how many it dropped;
- values interned in the AST by content (`std::meta::reflect_constant`
  template parameter objects, `define_static_string`/`define_static_array`
  objects, reflection handles): equal content gives the same entity in
  every run, so they carry no information between runs beyond what a
  run computes itself. They are the only kept state whose size can grow
  with the data runs see (`kept()` reports it).

**Accounting is a function of the run.** `steps` counts the run's own
execution. Semantic work a run triggers may evaluate nested constant
expressions (an instantiated body's `static_assert`); those run under
the translation unit's constant-expression limits
(`-fconstexpr-steps=` and friends), counted from zero, never charged
to the run and never cancelled by it, so the work, and so whether it
succeeds, does not depend on which run triggered it or how much of its
budget it had used. Together with the memo rule above, a run's steps,
and whether it exhausts a budget, are the same on run 1 and run 1000
and on a fresh or a used session.

## 6. Host calls

A run is not speculative: host calls are allowed (spec/host-calls.md
§4 "Runs"). The handler is **given per run**, borrowed (`&mut dyn
HostCalls`, any lifetime): the Oven reaches it through the run's bridge
(`SemaBridge::host_calls`), not through a registered handler. Nested
evaluations of semantic work the run triggers (§5) have no handler: a
host call there fails the evaluation with the note `no host-call
handler is registered`, as what they compute is kept across runs and
must not depend on one run's handler. The translation unit's handler
(given to the parse) is not used by runs.

## 7. Diagnostics

A run's diagnostics are returned in `RunReport::diagnostics` as
`kiln_diag::Diagnostic` records (the same records a sink receives:
id, level, message, span, notes), never printed. While a run is in
progress the session's `DiagEngine` captures every diagnostic instead
of sending it to its sink (`DiagEngine::capture_begin` /
`capture_end`): the error and warning counts and the fatal flag are
restored at the end of the run, and the run deduplicates only within
itself, so a run never sees the error limit, the counts or the
duplicates of another run or of the translation unit.

- **A failed run** reports `sema_immediate_call_non_constant` ("call
  to consteval function 'f' is not a constant expression") at the
  entry's declaration, with the evaluation's notes: the reason, then
  the call stack (`in call to 'g()'`, innermost first, under
  `-fconstexpr-backtrace-limit=`), as for any failed immediate
  invocation.
- **Replay.** Semantic work is kept (§5), but a fresh session would
  report its diagnostics in the first run that needs it. The session
  records the diagnostics emitted while the Oven compiles a function
  (its on-demand instantiation, `SemaBridge::ensure_body`) and while a
  reflection intrinsic runs (`SemaBridge::meta`, keyed by the op and
  its operands) or asks whether a function is non-throwing
  (`SemaBridge::is_nothrow`, which may instantiate an exception
  specification), and replays them, in place, in every later run the
  first time that run reaches the same function or makes
  the same query: a function with recorded diagnostics is set back to
  a stub at the end of each run, so its next call in a later run comes
  through the compile path, which replays and restores it without
  compiling again. A replayed error fails the run exactly as the
  original did (an invalid instantiation makes the call fail).
  Limits: diagnostics of semantic work done by a nested evaluation
  inside other semantic work are replayed with the outer work only;
  a reflection query whose operands are addresses (a string, a range)
  is keyed by the addresses.

## 8. Budgets, cancellation, errors

`RunBudget { steps, bytes, depth }`, given per run. Its `Default`
(`RunBudget::PROGRAM`) is the *program profile*: 2^34 steps, 1 GiB of
abstract memory, 16384 nested calls, against the constant-expression
defaults of 2^20 steps, 256 MiB and 512 calls. `RunBudget::with_steps`
etc. adjust one limit.

`CancelToken` (`Clone + Send + Sync`, an `Arc<AtomicBool>`): `cancel()`
from any thread stops a run that is given the token; `is_cancelled()`;
`reset()` makes it usable again (a cancelled token stays cancelled:
a run given one fails at its first check). The run checks the token
at least every 65536 steps (`CANCEL_INTERVAL`) of execution, so at
every loop back-edge and call within that many steps, and before each
host call: a `for (;;) {}` stops within a fraction of a millisecond.
The check rides on the step-limit check (spec/oven.md "Dispatch"), so
an evaluation without a session runs exactly the code it ran before.

`RunError` (`#[non_exhaustive]`), one kind per way a run ends:

| kind | when |
|---|---|
| `Failed` | not a constant expression (undefined behaviour, a call of a non-`constexpr` function, a leak, ...) |
| `StepBudget` | `budget.steps` exceeded |
| `MemoryBudget` | `budget.bytes` exceeded: an error, never an abort |
| `DepthLimit` | `budget.depth` exceeded |
| `Cancelled` | the token was cancelled; note `evaluation was cancelled` |
| `Host { message }` | the handler returned `Err` (`message` is its message; note `host call failed: <message>`), or the call's checks failed (spec/host-calls.md §2; `message` is the note) |
| `Uncaught { type_name, what }` | an exception left the entry: its type, and its `what()` when it derives from `std::exception` |

Every kind comes with its diagnostic (§7). After any of them the
session is usable and the next run behaves as on a fresh session.

## 9. Threading

`EvalSession: Send` (not `Sync`): a client may move a session to a
worker thread (a scoped thread, as it borrows the engine), one per
worker. `RunReport: Send`. `CancelToken: Send + Sync`. A `HostCalls`
handler is `Send` (spec/host-calls.md §3).

## 10. Cost

- **No session, no cost.** Without a session the Oven's dispatch is
  unchanged but for the step check comparing against
  `Oven::step_stop` (equal to the step budget outside a run) instead
  of the budget itself, and the compile path tests one `Option`;
  bench/oven counters (steps, evaluation bytes, accounted memory) are
  identical to the base's on all 29 entries, and wall times are within
  host noise (interleaved min-of-12: `wide-struct-read` 289 vs 284 ms,
  `vector-alloc` 414 vs 405 ms, `-fsyntax-only` of `<bits/stdc++.h>`
  1254 vs 1018 ms, base vs WP-1995).
- **Retained memory and per-run overhead**, measured by the ignored
  test `measure` in `crates/kiln_driver/tests/eval_session.rs` (a
  release build, `-- --ignored --nocapture measure`; WP-1995, aarch64
  Linux development host; indicative, not a gate):

  | | small transform | `<bits/stdc++.h>` + transform |
  |---|---|---|
  | parse and open | 49 ms | 1.16 s |
  | kept (`kept().total()`) after open | 35 KiB | 60 MiB (AST 52 MiB, Oven 8 MiB, Sema 0.6 MiB) |
  | process RSS growth to open | 11 MiB | 77 MiB |
  | first run of an empty entry (open to first instruction) | 30 us | 14 us |
  | an empty entry's run | 71 ns | 76 ns |
  | runs/s of `return 1;` | 11.4 M | 10.7 M |
  | 50,000 runs of 100 host calls | 0.44 s (8.7 us/run), kept flat | 0.44 s (8.9 us/run), kept flat |
  | cancellation of `for (;;) {}` from another thread | 0.8 ms | 0.1 ms |

  A `std::vector<int>` of 10^6 `push_back`s: 0.46 s and 3.8e7 steps
  per run, peak 6 MiB; `kept()` after run 1000 equals `kept()` after
  run 2 and the process RSS does not move between them.
- **Flat over runs.** A run records no evaluation statistics
  (`-fconstexpr-report` rows) and its entry stub is compiled once; the
  frame and heap spaces' buffers are released after a run down to
  1 MiB each (`RELEASE_FLOOR`), so a run that built a large structure
  does not hold its memory. `kept()` after run 1000 equals `kept()`
  after run 2 for an entry whose allocations do not depend on the run.

## 11. Precompiled headers and modules

A session never writes a precompiled header or a module file, and a
session's translation unit is parsed without recording either.

## Tests

- `crates/kiln_parse/tests/eval_session.rs`: every
  rule above, with a Rust handler: isolation (a host call returns
  different data each run; nothing a run builds is visible to the
  next), each budget (N-1 fails, N passes, fresh and used), memo
  hit or miss same steps on run 1 and 1000, cancellation from another
  thread, every failure kind followed by a clean run, host calls in
  runs, diagnostics and their replay, kept memory flat, every entry
  error, a refused translation unit.
- `crates/kiln_driver/tests/eval_session.rs`: `with_eval_session` from a
  command line (an include path and a define, a refused translation
  unit).
