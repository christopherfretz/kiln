# Evaluation sessions (WP-1995)

Status: implemented (WP-1995; nested work, §5.1, WP-2073). Code: `crates/kiln_sema/src/session.rs`
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
- **The driver's stages** (WP-2120). `with_eval_session` runs the
  command line through the front-end library entry's stages
  (`front_end::prepare`, spec/driver.md "Front-end library entry"): the
  same checks, toolchain, driver warnings, `-v` block, standard and job
  as `kiln -fsyntax-only`, with the action forced to `-fsyntax-only`
  (so `-c` / `-o` on the line are accepted and nothing is written), the
  PCH probe off, `-verify` and `-include-pch` refused, the module cache
  read only, and the process environment.

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
| `steps` | steps the run's own execution took (spec/oven.md "Limits"), §5 |
| `nested_steps` | steps of nested evaluations charged to the run, fresh and replayed (§5.1; WP-2073) |
| `host_calls` | handler calls the run made |
| `peak_heap` | the run's peak live abstract memory (frame and heap bytes; spec/oven.md `peak_bytes`), charged nested peaks included (§5.1) |
| `calls` | function calls the run's own execution made |
| `nested_calls` | function calls of nested evaluations charged to the run (§5.1) |
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
  it changes no counter and no budget decision (a hit is not taken
  where it would cross a pending step check, §5.1). `clear_memo()` drops
  every entry and returns how many it dropped;
- values interned in the AST by content (`std::meta::reflect_constant`
  template parameter objects, `define_static_string`/`define_static_array`
  objects, reflection handles): equal content gives the same entity in
  every run, so they carry no information between runs beyond what a
  run computes itself. They are the only kept state whose size can grow
  with the data runs see (`kept()` reports it).

**Accounting is a function of the run.** `steps` counts the run's own
execution; nested work is charged by §5.1. A run's steps, nested steps
and calls, and whether it exhausts a budget, are the same on run 1 and
run 1000 and on a fresh or a used session, whatever other entries ran
before and in whatever order (the residuals at landing, work of kinds
that are not units yet, are listed in §5.1).

### 5.1 Nested work (WP-2073)

**Terms.** A *nested evaluation* is an evaluation context opened while
a run is in progress by semantic work, not by the run's own code: every
context `Oven::eval_nested` opens (`kiln_oven/src/lib.rs`), and the
Oven's own initialization of a variable it reads (`eval_var_init`,
`kiln_oven/src/value.rs`). A *unit* is a piece of kept semantic work
that is done once per session and may open nested evaluations. The
units at landing (`Key` in `kiln_sema/src/session.rs`):

| unit key | the work | reached at |
|---|---|---|
| `Func(f)` | the Oven's compilation of function `f`, with the semantic work it needs: an instantiation, an implicit or defaulted member, a metafunction thunk (`SemaBridge::ensure_body`) | every call of `f` in a run (`Oven::session_compile`) |
| `Var(g)` | the Oven's evaluation of global `g`'s initializer for a read (`eval_var_init`) | every first read of `g` in a run (`Oven::session_kept_var`) |
| `Init(v)` | a variable's initializer instantiated and checked (`instantiate_var_spec_init`, `subst_static_member_init`) | each reference to `v` that may need its value (`note_var_reference`) |
| `Inst(f)` | a function template specialization's definition instantiated (`instantiate_function_definition`) | each request to instantiate it, done or not (its early return, `note_used_for_instantiation`) |
| `Meta(op, operands)` | a reflection query (`SemaBridge::meta`) | each query with the same op and operands |
| `Nothrow(f)` | whether `f` is non-throwing, which may instantiate an exception specification (`SemaBridge::is_nothrow`) | each query, the early `known_nothrow` answer included |

A unit is *reached* each time a run, or a unit's work in it, asks for
it: the call that does it, and every later request that finds it done.

**Rule 1: atomic under the translation unit's limits.** A nested
evaluation runs to completion under the translation unit's
constant-expression limits (`-fconstexpr-steps=` and friends, fixed for
the session), counted from zero, never cancelled, never under the
run's budget, and with no host handler (§6). Its outcome, and so the
kept work, is a function of the translation unit alone. The reason is
that semantic work cannot be rolled back: what a nested evaluation
computes flows into kept state (an `if constexpr` branch instantiated,
a template argument naming a specialization, a constraint choosing an
overload, a `constexpr` variable's value), and kiln has no transaction
over semantic work. Stopping one part-way by the run's budget or token
would keep a failure that a later run with a larger budget would
see; so nested work is never budget- or cancel-dependent. Nested work
that exceeds the translation unit's own limits fails as it would in the
translation unit: that failure is kept, and every run that reaches it,
on a fresh or a used session, fails the same way (`RunError::Failed`
with the same diagnostics, §7).

**Rule 2: recorded once.** While a unit's work runs (in whichever run
first reaches it), the session records with it, beside its diagnostics
(§7): its *own cost*, the cost of the nested evaluations it opened
itself (`NestedCost`: steps and calls summed; peak live bytes, peak
address space and call depth, each counted from zero by rule 1, as
maxima), and its *children*, every recorded unit reached during its
work, in order. A child reached already done is a child all the same.
A unit that opened nothing, reached no recorded unit and has no
diagnostics has no record and costs nothing to track.

**Rule 3: charged at the reach, once per run.** The first time a run
reaches a recorded unit from its own code, it is charged `cost(u)`:
`u`'s own cost plus `cost(c)` of each child `c` not yet charged in
this run, recursively (steps and calls summed, the rest as maxima).
Fresh work is charged the same way, from its record, when its work
ends. A reach inside a unit's work is only recorded (as a child); its
charge is part of the enclosing unit's. Steps and calls go to
`nested_steps` and `nested_calls`. The run's **budget decision is made
on its own cost plus its charged nested cost**, alike for a run that
does the work and one that replays it:
- `steps + nested_steps` above `budget.steps` fails the run with
  `StepBudget` (the run's own steps may use what its nested work
  left);
- the address space in use at the reach plus the charged peak address
  space above `budget.bytes` fails it with `MemoryBudget`; the live
  bytes at the reach plus the charged peak live bytes enter
  `peak_heap`;
- the active calls at the reach plus the charged depth above
  `budget.depth` fails it with `DepthLimit`.

The failure is reported at the run's next step check (the next
statement executed), with the step-limit, memory-limit or depth note
and the call stack of that point; a charge after the run's last step
check fails the run at its end, with the note at the entry. A
memoized call is not taken where it would cross a pending step check
(`c.steps + v.steps > step_stop` in a session), so a run charged over
its budget stops at the same step whether or not an earlier run warmed
the memo.

**Why it is deterministic.** The set of units a run reaches from its
own code is a function of the run's execution; each unit's record is
a function of the translation unit (rule 1); a unit's children are
what its work asks for, which does not depend on what was done before
(a child done earlier is still reached); and charging a set of units
charges each once, so neither order nor history changes the sum.
Checked by `crates/kiln_parse/tests/eval_session_nested.rs`, where a
fresh session per run is the reference:
- **Run 1 and run N, fresh and used, A then B and B then A**, with
  nested work inside nested work (a static member's initializer whose
  evaluation instantiates a function that reads another static
  member) shared by two entries, memoized calls and a diagnostic:
  steps, `nested_steps`, calls, `nested_calls`, the result and the
  diagnostics are identical.
- **A budget the nested work exceeds.** The work completes under rule
  1 and is kept; the run is charged it and fails with `StepBudget`. A
  sequence of runs with that budget, then a budget one step short of
  the total, then the exact total, gives the fresh session's report
  each time; nothing is charged part-way, nothing is kept half-done,
  nothing is redone.
- **Depth and bytes**: nested work's depth and address space count on
  top of the run's, alike fresh and used.
- **The memo.** A function whose compilation was recorded stays a stub
  while the session is open, so each of its calls goes through the
  compile path, which reaches the record and makes every call in
  progress impure: no memoized result skips a charge or a replay.

**Reach observation.**
- `Func`: a recorded function's real state is kept aside
  (`SessionState::tracked`); every call reaches it and bumps
  `run.impure`, and the first call in a run outside nested work, or
  any call inside it, reports the reach.
- `Var`: a global whose initializer's evaluation was recorded keeps its
  object and is made unresolved again at the end of every run, so the
  next run's first read reaches the record.
- The sema units: the hooks test one `Option` (`Sema::eval_replay`,
  set only inside a run).

**Not hooked at landing (WP-2075).** A class template
specialization's instantiation, an exception specification's
instantiation outside `is_nothrow`, an atomic constraint's
satisfaction and a default member initializer's instantiation are not
units. Their work, its nested evaluations and its diagnostics, belong
to the unit whose work first did them. Undercharged meanwhile: a later
unit (in the same or a later run) that needs such work already done is
charged neither its nested cost nor shown its diagnostics, so a run
that reaches it through a different unit than a fresh session would
reports less `nested_steps`/`nested_calls` and can miss its
diagnostics. Other kept work that is not a unit (a default argument's
instantiation, an alias substitution's cache, a reflection table
entry) is the same residual (WP-2076). So is the reflection-bytes
charge (spec/oven.md "Limits": a value already interned charges 0),
which makes a run's peak bytes depend on what earlier runs interned
(WP-2077). The nested limits are the translation unit's; a
session-level setting for them is WP-2078.

**Unchanged.** The translation unit's own nested evaluations, before
any run, run under its limits and its handler, are not units and are
never charged to a run. Without a session nothing here runs: the sema
hooks test one `Option`, the Oven's step check and compile path are
as before, and bench/oven counters stay identical.

## 6. Host calls

A run is not speculative: host calls are allowed (spec/host-calls.md
§4 "Runs"). The handler is **given per run**, borrowed (`&mut dyn
HostCalls`, any lifetime): the Oven reaches it through the run's bridge
(`SemaBridge::host_calls`), not through a registered handler. Nested
evaluations of semantic work the run triggers (§5.1) have no handler:
a host call there is refused, as what they compute is kept across runs
and must not depend on one run's handler. The translation unit's
handler (given to the parse) is not used by runs.

The refusal's note is `host call in semantic work a run triggered (its
result is kept across runs)`, and the refused call bumps `run.impure`
as every refused call does (spec/host-calls.md §5), so no call around
it is memoized. It is a structured failure of the unit (§5.1 rule 1),
kept and replayed like any other diagnostic (§7), so every run that
reaches the work gets it identically, with the same `RunError` kind
(`Failed`), on a fresh or a used session alike: a static member's
initializer that makes a host call reports its error with that note
once, records it, and every run that reaches the initializer replays
it. A client that needs the handler
calls it from the run's own code (the entry's body and what it calls),
never from a `static_assert`, a `constexpr` variable or a template
argument of an instantiation.

Not chosen: re-doing such work with the run's handler in every run.
The value of a nested evaluation flows into kept state that cannot be
re-derived per run (§5.1 rule 1); kiln has no transaction over semantic
work (the Oven's trials undo only abstract memory, spec/oven.md "Trial
evaluation").

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
  records the diagnostics emitted during each unit's work (§5.1), in
  the same tree as its cost: a unit's segments are its own diagnostics
  and its children, in order. They are replayed, in place, the first
  time a run reaches the unit, from its own code or from another
  unit's work; a unit reached again in the same run replays nothing.
  Sema holds the run's recording for the run's duration
  (`Sema::eval_replay`), so the bridges of semantic work done by a
  nested evaluation inside other semantic work record into it too: work
  nested inside nested work is a unit of its own, replayed wherever it
  is reached. A replayed error fails the run exactly as the original
  did (an invalid instantiation makes the call fail).
- **Context notes.** A diagnostic recorded inside an instantiation
  carries that instantiation's context notes (`in instantiation of
  ...`, `required from here`). On replay the recorded context is
  replaced by the context of the reach: what a fresh session would
  print at the point the run reached the unit. Recording does not use
  up the context notes of the diagnostics that follow.
- **Limits.** A reflection query's record is keyed by the query
  (below); the units its work reaches are keyed by unit, so a different
  query, or a run's own code, that needs the same already-done unit
  replays it. Work of the kinds that are not units at landing (§5.1,
  WP-2075; a class instantiated by `substitute`, then needed by another
  query) is replayed with the first unit or query that did it only
  (WP-2160). A replayed diagnostic has the location of its recording;
  only its context notes are those of the reach.
- **Keys are content.** No replay key, memoization key or kept result
  depends on an address or on what earlier runs did. A reflection
  query is keyed by its op, the context it reads (whether it is a
  trial and whether it runs inside semantic work; for the ops that
  read them only, the evaluation point's scope, the nested evaluation
  point, whether the evaluation is plainly constant-evaluated) and each
  operand as the intrinsic reads it, read through the same Oven
  accessors (`crates/kiln_sema/src/meta/key.rs`; every operand of
  every op has a row there, checked by a unit test): an `info`, an
  integer or an `access_context` by its bits; a range of `info`
  (`substitute`'s arguments, a trait's types, `define_aggregate`'s
  descriptions) by its length and elements; a `(const info *, n)`
  trait form by its `n` elements; `reflect_constant_string` /
  `reflect_constant_array`'s range by its element type, count and
  elements frozen as the op freezes them (a content-interned constant);
  `reflect_constant`'s operand frozen at its type; `reflect_object` /
  `reflect_function`'s address frozen as a pointer (the entity and
  offset, not the encoding); `is_string_literal`'s by its answer;
  `data_member_spec`'s options by what is read from them (name bytes,
  alignment, width, `no_unique_address`, annotations); a name by its
  bytes; a value op's specialization types. An operand the op rejects
  by its type alone keys as rejected; a read that fails ends the
  operand (the op stopped reading there too). The encoding is exact
  (lengths before elements, the whole key compared), so equal content
  hits the same record from any address and different content never
  does. The key is computed after the call and only when needed: the
  call reported something, did nested work or reached a recorded unit
  (§5.1), or some call of the same op has a record. A query whose key
  has a record and that records nothing new replays and is charged
  that record (`Replay::end`), as any unit reached again. What
  computing a key interns (a value the op did not freeze, as it
  failed first) is not charged to the run (`Oven::meta_uncharged`).
- **Found and fixed by WP-2073** (each with a test in
  `crates/kiln_parse/tests/eval_session_nested.rs`): a call memoized
  after a recorded function was reached let a later run skip its
  replay (the function now stays a stub, §5.1); work shared by two
  units was replayed with the first only (`Init` and `Inst` units); an
  initializer read under a small run budget kept its failure for every
  later run (`eval_var_init` now runs under rule 1). An exception
  specification resolved by one run was answered early
  (`known_nothrow`) in the next without its replay: not reproduced
  (sema resolves the specification at the use, inside the unit that
  compiles the caller, so the query finds no recorded work; the tests
  pin the behaviour for a direct call, a member pointer and a virtual
  call), fixed all the same: every reach of a recorded unit must be
  seen, and the early answer now reaches a recorded `Nothrow` unit.
  The entry stub compiled before the run is armed does no semantic
  work (`compile_call` only resolves the callee's index): not a bug.

**Keys and caches a run reads or fills** (WP-2074). "Address" is a
frame or heap encoding (slot | generation | offset), which `reset`
reuses in every run; a static encoding names one object for the
session (static slots are reused only after a scratch object is freed,
with the next generation). Ids interned on first sight (`ReflectionId`,
`ConstValueId`, an Oven function index) are numbered by history but
are a bijection with their content for the session's lifetime, so
they key exactly.

| key or cache | site | key holds | why it is exact |
|---|---|---|---|
| replay `Key::Meta` | `kiln_sema` `session.rs` `Replay::records`; `oven_bridge.rs` `meta` | content (above) | fixed by WP-2074; tests `run_built_operands_replay_by_content`, `a_run_does_not_depend_on_the_runs_before_it` |
| replay `Key::Func` | `session.rs`; `kiln_oven` `session.rs` `tracked` | Oven function index | one index per function for the session |
| replay `Key::Var` | `kiln_oven` `session.rs` `kept_vars`, `value.rs` `eval_var_init` | Oven global index | one index per global for the session |
| replay `Key::Init`, `Key::Inst` | `kiln_sema` `templ/class.rs`, `templ/var.rs` | `DeclId` | no address |
| replay `Key::Nothrow` | `oven_bridge.rs` `is_nothrow` | `DeclId` | no address |
| memoization | `kiln_oven` `memo.rs` `MemoKey`; eligibility `compile.rs` (`memo =`) | function index, flags, up to 4 scalar arguments | only integer, `bool`, floating and `info` parameters, no `this`, no pointer result: a pointer or reference argument (an address) makes the function not memoizable; an `info` of an automatic variable is a handle to the running evaluation's frame, so `extract` and `constant_of` of one mark the calls in progress impure (fixed by WP-2074: a call given `^^x` was memoized by its bits and read a stale `x`); otherwise a memoized function reaches only static objects; tests `memoized_results_are_not_keyed_by_addresses`, `reflections_of_locals_are_not_memoized`, `tests/conformance/meta.reflection.extract/local-reflection-argument-memo.cpp` |
| `substitute` memo | `kiln_sema` `meta/templ.rs` `SubstMemo` | template and argument reflection bits | the arguments are read from the range before the lookup |
| member lists | `kiln_oven` `meta/member_cache.rs` `MemberCache` | `DeclId` and list kind | no address |
| union path memo | `kiln_oven` `vm/umemo.rs`, `Memory::union_epoch` | allocation key (slot, generation) | an entry is trusted only at the epoch it was computed at; every change to the union state bumps the epoch, and `reset` bumps it when it drops a run's union entries, so an entry a later run can hit was computed with no union state of that object, a function of the types and offset (also in the key) |
| union designators | `vm/udesig.rs` | (designator, type) interned | content |
| address/integer cells | `kiln_oven` `addr_int_disps`, `label_diff_cells`, `synth` | (space, address) | written only when folding C static initializers (`set_fold_c_static`, codegen), never in a session |
| thawed constants, literal objects, globals | `thawed`, `lit_exprs`, `lets`, `unknowns`, `provisional_objs`, `global_map` | `ConstValueId`, `ExprId`, `DeclId`, global index | values are static addresses, valid for the session |
| thunks, compiled code | `thunks`, `thunks_at`, `func_map`, `ctor_steps` | `ExprId`, `DeclId` | no address |
| member pointers, virtual calls | `mp`, `virt` | interned members, class and function types | content |
| data member descriptions | `kiln_oven` `meta/define.rs` `DmsTable` | the description (`Dms`), annotation lists | content |
| host-call taint | `kiln_sema` `consteval.rs` `host_failed` | `ExprId` | filled only when a handler call is made outside a run; nested evaluations of runs have no handler |
| entry stubs | `kiln_oven` `session.rs` `stubs` | `DeclId` | no address |
| reflection charge | `kiln_oven` `meta/intern.rs` `charge_refl` | interned growth | not a key: what an intrinsic call is charged depends on whether its result was already interned (spec/oven.md "Reflection bytes") |

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

**Nested work (WP-2073).** A nested evaluation is never stopped by
the token or the run's budget (§5.1 rule 1): semantic work cannot be
rolled back, so stopping it would leave its unit half-done and keep its
failure for every later run. Budget and cancellation take effect at
the run's next step check after the nested work returns. **The
cancellation latency** is therefore bounded by the remaining nested
evaluations of the unit in progress, each capped by the translation
unit's step limit (`-fconstexpr-steps=`): a client that cancels a run
inside a large instantiation waits for that instantiation's
evaluations, never for more than their count times that limit. A run
whose charge exceeds a limit fails with `StepBudget`, `MemoryBudget` or
`DepthLimit` (§5.1 rule 3); one cancelled during nested work fails with
`Cancelled`, and the work stays done exactly as a fresh session would
have done it. The Oven's own initialization of a variable it reads
(`eval_var_init`) is a nested evaluation too and follows the same
rule.

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
- **Content keys** (WP-2074), measured by the ignored test
  `measure_content_keys` (same file; `--profile harness`, aarch64 Linux
  development host, min of 20 runs): a run making 2000 `substitute`
  calls, each over a `std::vector<info>` it built, takes 4.65 ms while
  no `substitute` call has a record (no key is computed), 4.76 ms (+2.4%,
  about 55 ns per call) once one has (every call computes its content
  key), and 4.92 ms when half the calls also hit the record and replay
  it. A record's key words count in `kept().replay`. Without a session
  no key is computed: bench/oven counters (steps, evaluation bytes,
  accounted memory) are those of the base on all 29 entries (none
  reflects; the only change outside sessions is that `extract` and
  `constant_of` of an automatic variable mark the calls in progress
  impure, spec/oven.md "Reflected locals").
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
- `crates/kiln_parse/tests/eval_session_nested.rs`: §5.1 (WP-2073):
  each fixed bug of §7, the determinism criterion (run 1 and run N,
  fresh and used, both orders, nested inside nested work, the memo),
  the budget decision on own plus charged nested steps, depth and
  bytes, cancellation, a host call refused in nested work, and nested
  work over the translation unit's limit.
- `crates/kiln_driver/tests/eval_session.rs`: `with_eval_session` from a
  command line (an include path and a define, a refused translation
  unit); content keys (WP-2074): a query over operands a run built
  (`substitute` of a vector, of a `define_static_string`, of a filtered
  `members_of`) replays its warning in a later run that builds them at
  another address and not in one that builds other content at the same
  address, a run after others reports what a fresh session's would, and
  the memoization cache skips pointer and reference arguments.
- `crates/kiln_sema/src/meta/key.rs` (unit tests): every operand of
  every reflection op has a key row.
