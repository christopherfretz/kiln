# Host calls (WP-1990)

Status: implemented (WP-1990). Code: `crates/kiln_oven/src/host.rs` (the
Rust interface and the modes), `crates/kiln_oven/src/vm/host.rs` (the
call), `crates/kiln_oven/src/compile/builtins.rs` (`bi::HOST_CALL`),
`crates/kiln_sema/src/consteval.rs` (failures not evaluated again),
`crates/kiln_sema/src/builtins.rs` (visibility), `crates/kiln_pp/src/features.rs`
(`__has_builtin`), `crates/kiln_codegen/src/builtins.rs` (run-time calls),
`crates/kiln_driver/src/host_test.rs` (the test handler).

A tooling client that drives kiln as a library can let constant
evaluation call into Rust: it registers a handler, and
`__builtin_kiln_host_call` forwards each call to it. The builtin is
generic: kiln knows nothing of the operations a handler implements, so a
client adds operations without changing kiln. Nothing here changes what
kiln does for a translation unit that registers no handler.

## 1. The builtin

```cpp
unsigned long long
__builtin_kiln_host_call(unsigned op,
                         const unsigned long long *in, unsigned n_in,
                         const char *bin, unsigned n_bin,
                         unsigned long long *out, unsigned cap_out,
                         char *bout, unsigned cap_bout);
```

- `op` selects the handler's operation. `in`/`n_in` are the integer
  arguments, `bin`/`n_bin` the byte arguments (names, text), `out`/
  `cap_out` the integer results and `bout`/`cap_bout` the byte results.
  Every length is a count of elements, given by its argument: no
  terminator is looked for, so a byte argument may hold a NUL.
- The result is the handler's `u64` and kiln does not interpret it. A
  client's convention (count-then-fill, a status code) is the client's.
- **Visibility.** The builtin exists only in a translation unit that has
  a handler registered (§3). Then `__has_builtin(__builtin_kiln_host_call)`
  and `__has_constexpr_builtin(__builtin_kiln_host_call)` are 1 and the
  name is implicitly declared like any builtin (an `extern "C"`
  `noexcept` function). Without a handler both are 0 and the name is an
  ordinary undeclared identifier: a call is the usual "use of undeclared
  identifier" error, and a program that tests `__has_builtin` first is
  unaffected.
- The builtin's name follows clang's `__builtin_` prefix for builtin
  functions, so `__has_builtin` reports it like the others.

## 2. Semantics of a call

A call is evaluated only inside a constant evaluation that may have
effects (§4). It proceeds in this order; a failed step ends the call
before any later step runs:

1. **Charge.** `1 + n_in + n_bin` steps (spec/oven.md "Limits and
   metrics"). Exceeding `-fconstexpr-steps` is the usual step-limit
   failure, and the handler is not called.
2. **Check every range.** For each of `in`, `bin` (read) and `out`,
   `bout` (write) with its length `n`:
   - a length of 0 accepts any pointer value, null included, and the
     pointer is not used;
   - otherwise the pointer is not null and designates `n` consecutive
     elements of one array (or the single object, `n` = 1) of the
     parameter's element type;
   - `in` and `bin`: each of the `n` elements is within its lifetime and
     initialized (each is read, in order, into a buffer of the call);
   - `out` and `bout`: the elements are writable in this evaluation:
     not `const`, not a string literal, and an object whose lifetime
     began within the evaluation ([expr.const]/5.16).
   A failed check makes the call not a constant expression, with the
   note `'<arg>' of __builtin_kiln_host_call <reason>` naming the
   argument (`in`, `bin`, `out`, `bout`) and the call stack. The handler
   is **not** called: a handler has effects, so a call never half-runs.
   In an evaluation-session run this is kiln's refusal, not the
   handler's error: the run fails `RunError::HostArgument` (WP-2480,
   spec/eval-session.md §8).
3. **Call the handler** with `op`, the integers read, the bytes read and
   a `HostOut` holding the two capacities (§3). The inputs were read
   before any output is written, so an output range that overlaps an
   input range is well defined.
4. **Result.** `Ok(v)`: if the handler wrote more integers or bytes than
   their capacity, the evaluation fails ("host call handler wrote N
   integers (bytes) into a capacity of M"). Otherwise exactly the
   elements written are stored, as by assignment, at the start of `out`
   and `bout`, and charged one step each; the other elements are left as
   they were (reading one that was never initialized is the usual
   uninitialized-read failure). The call's value is `v`.
   `Err(e)`: the evaluation ends, uncatchably, with the note
   `host call failed: <message>` at the call, and the call stack (in a
   session's run, `RunError::Host`, as for a handler that wrote past a
   capacity: the handler's own error). No
   `catch` sees it and no destructor of the evaluation's objects runs,
   so nothing the evaluation would still do reaches the handler. A
   client that wants a failure a program may handle encodes it in the
   result (a status code) and its own library throws.

## 3. The Rust interface

In `kiln_oven` (re-exported by `kiln_sema` and `kiln_parse`):

```rust
pub trait HostCalls {
    fn call(&mut self, op: u32, ints: &[u64], bytes: &[u8], out: &mut HostOut<'_>)
        -> Result<u64, HostError>;
}
```

- `HostOut::int_capacity()` / `byte_capacity()` are `cap_out` and
  `cap_bout`; `write_ints(&[u64])` / `write_bytes(&[u8])` append to the
  results; `ints_written()` / `bytes_written()` count them. Appending
  past a capacity is kept (the call then fails, §2.4), never truncated.
- `HostError::new(message)`; `message()`. It implements `Display` and
  `std::error::Error`.
- Registration: `Oven::set_host_calls(Option<Box<dyn HostCalls + Send>>)`
  (returns the previous handler), `Sema::set_host_calls` (also makes the
  name visible, §1), and `kiln_parse::parse_preprocessed_with_host`,
  which registers the handler for one translation unit and hands it back
  in `ParsedTu::oven` (`Oven::take_host_calls`). The preprocessor's
  `__has_builtin` answer is set separately
  (`ClangFeatures::with_kiln_host_calls`), as the preprocessor runs
  before sema exists; the two must agree.
- `Oven::host_call_count()` counts the calls the handler received.
- **`Send` is the registration's bound, not the trait's** (WP-2321).
  The trait has no supertrait. A handler that is *stored* (registered
  with `Oven::set_host_calls`, `Sema::set_host_calls`, given to
  `parse_preprocessed_with_host` or `parse_preprocessed_retained`, kept
  as `EvalSession::take_tu_host_calls`) is a `Box<dyn HostCalls + Send>`,
  so an Oven (and a `ParsedTu`, an `EvalSession`) holding one can still
  be moved to another thread; an Oven is used by one thread at a time,
  so nothing needs `Sync`. An evaluation-session run's handler
  (`EvalSession::run(.., host: &mut dyn HostCalls, ..)`) is borrowed for
  the call and never stored (spec/eval-session.md §6), so it may be any
  handler: one holding an `Rc`, a `RefCell` or a borrow of another
  unit's `Ast` (`Ast: Sync` holds, spec/ast.md §2.2, but a handler need
  not rely on it). Test: `crates/kiln_parse/tests/eval_session.rs`
  `a_runs_handler_need_not_be_send`.
- **Zero cost when unregistered.** Without a handler no call of the
  builtin can be compiled (the name is undeclared). What remains on the
  other paths is saving and restoring the host mode (§4) at each
  evaluation entry point, a test of one flag where sema checks an
  immediate invocation, and one comparison in the dispatch of the
  Oven's builtin instruction.

## 4. Where a call may run

A host call has an effect outside the evaluation, so it runs only in an
evaluation whose result is used: one that must be a constant expression
and is evaluated once where the program says. Kiln's entry points
(spec/oven.md "Evaluation entry points") fall in three classes:

- **Runs.** A manifestly constant-evaluated expression the program
  requires to be constant: `Oven::evaluate` (a `constexpr` variable's or
  `constinit` initializer, a `static_assert`, an immediate invocation, a
  template argument, an array bound, a `case` label, `if constexpr`) and
  `Oven::evaluate_chars` (a `static_assert` message), and an immediate
  invocation in a default argument codegen evaluates at its use
  (`Oven::fold_immediate_in`). A nested evaluation inside one of these
  inherits its class, except as below.
- **Refused silently** (the evaluation fails with no note and the
  handler is not called): the trial folds sema and codegen use for
  warnings, narrowing, constant folding of run-time code and the
  potential-constant-expression check of a `constexpr` function
  definition (`try_fold`, `fold_rvalue`, `fold_manifest`,
  `fold_object_size`, `fold_for_overflow`, `warn_constant_evaluated`,
  `check_contract_init`, `evaluate_call`, `evaluate_potential`,
  `has_constant_destruction`). Such a fold is "not constant", as for
  any other effect it cannot have.
- **Speculative: an error.** An evaluation that asks whether something
  is a constant expression to decide what the program means is refused
  with the error `host call during speculative evaluation` at the call,
  and the handler is not called:
  - whether a variable with static or thread storage is
    constant-initialized (`fold_constant_init`, `is_constant_initialized`):
    a namespace-scope `const unsigned long long g =
    __builtin_kiln_host_call(…);`. For a block-scope variable the same
    question, which sema asks as it classifies a use of the variable, is
    refused silently: the variable is not usable in constant expressions
    and is read from its frame when the function runs;
  - a nested evaluation of a variable's initializer for a read
    (`int a[n];` with `const unsigned long long n = …call…;`: the value
    of `n` would be computed by a second, hidden evaluation);
  - the argument of `__builtin_constant_p` (a speculative trial);
  - a transactional trial (`std::meta::can_substitute`), which may be
    rolled back;
  - the `enable_if` condition (`evaluate_with_params`).

**A failed call decides nothing in a SFINAE context** (WP-2501). A
call that ran in an allowed evaluation (a template argument, an array
bound, a constraint's atom) and failed with the handler's error, its
results over a capacity, no handler, or an evaluation session's refusal
(spec/eval-session.md §6: outside a run, and in a run's nested work) is
flagged (`Oven::host_failed`; a range check of the arguments,
§2.2, is not: that is the program's own fault). When the evaluation
that failed so was in a SFINAE context (a requires-expression's
requirement, a function template's deduction, a partial
specialization's match, a constraint's substitution), its error is not
a substitution failure but a hard error, printed with the failure's
note (`host call failed: …`, the session's refusal note) and the
context notes, the requirement's (`in instantiation of requirement
here`) or the deduction's among them (spec/sema-templates.md §5.7):
whether the handler answers is no property of the template arguments,
so a refused call never makes a requires-expression `false` or skips a
candidate. An `if constexpr` condition is no SFINAE context; it was
already the same error. Tests:
`tests/conformance/ext.kiln.host_call/host-call-sfinae.cpp`,
`crates/kiln_driver/tests/eval_session.rs`
`a_refused_host_call_is_no_substitution_failure` (the session's
translation unit, refused) and
`crates/kiln_parse/tests/eval_session_nested.rs`
`a_host_call_refused_in_a_requires_expression_is_an_error` (a run's
nested work).

Consequences, by construct:
- A function definition never calls the handler when it is parsed. In
  a `consteval` or `constexpr` function, `const unsigned long long n =
  __builtin_kiln_host_call(…);` used only as a value is evaluated once
  each time the function is evaluated.
- A block-scope `constexpr` variable is a constant expression evaluated
  once, at its definition (spec/oven.md "Constexpr locals"): its call
  runs when the function is defined, not when it runs.
- Unevaluated operands (`sizeof`, `decltype`, `noexcept`, `typeid` of a
  non-polymorphic operand, a `requires` expression's) call nothing.
- **Run time.** A host call is never code. A function that reaches one
  and is emitted (a `constexpr` function called at run time) is the
  compile error `host call outside constant evaluation` at the call,
  never an undefined symbol.

## 5. Memoization

A host call is impure: its result depends on the handler, not on the
arguments. Each call bumps the Oven's impurity count
(`Oven::impure_count`), so no call frame that is pending when it runs is
entered into the memo (spec/oven.md "Memoization"), and no evaluation is
reused: a function evaluated twice with the same arguments calls the
handler again, and so does each top-level evaluation. A refused call
(§4) bumps it too.

A failure is not evaluated again for its diagnostics either. A failed
immediate invocation of a `constexpr` or `constinit` variable's
initializer is reported after the variable's own check, and both would
evaluate it again; when the failed evaluation called the handler, sema
keeps its failure and both reports reuse it (outside a template, whose
instantiations each evaluate their own). The diagnostics are those of the
one evaluation.

Each call stack that reaches a refused speculative call (§4) is reported
once: the same evaluation tried twice is one error, two places that
reach the call are two.

A `constexpr` variable whose constant initialization failed is reported
once (WP-2330). A read of it re-folds its initializer non-manifestly
(`refold_var_init`, clang's `VarDecl::evaluateValue` after a failed
check), and a host call in that re-fold is refused silently: the
failure is the variable's own error (`constexpr variable 'v' must be
initialized by a constant expression`, with the call's failure,
speculative refusal or nested-work refusal as its note, spec/eval-session.md
§6), so the read adds only its own `initializer of 'v' is not a constant
expression`, never a second `host call during speculative evaluation`.
A `const` variable that is not `constexpr` keeps the speculative error
in that re-fold (its constant-initialization check may have been a
silent classification, §4). Test: `crates/kiln_parse/tests/host_calls.rs`
`a_failed_constexpr_variable_is_reported_once`.

## 6. Precompiled headers and modules

A handler belongs to the process, not to a translation unit, so nothing
a host call produced may outlive the translation unit:
- `Oven::snap_ready` fails while a handler is registered
  (`Oven.host: a host-call handler is registered`), so no precompiled
  header snapshot is written;
- sema records no carried constant results for a module interface
  while a handler is registered;
- the driver rejects its test handler (§7) with `-emit-pch`, `-x
  c++-header` output, `--precompile` and `-fmodule-output`
  (`drv_host_calls_artifact`).
Reading a precompiled header or importing a module with a handler
registered is unaffected: what they carry was made without one.

## 7. The test handler

The hidden driver flag `-fkiln-host-call-test-handler` (kiln's own tests
only; not a supported interface) registers this handler and makes the
builtin visible:

| op | does |
|---|---|
| 0 | returns the number of handler calls before this one |
| 1 | returns a counter, then increments it |
| 2 | writes `ints` and `bytes` back to the outputs, returns `(ints.len() << 32) \| bytes.len()` |
| 3 | fails with `boom` |
| 4 | returns `cap_out << 32 \| cap_bout` |
| 5 | sets the cancellation flag, returns 0 |
| 6 | fails with `cancelled` if the flag is set, else returns 0 |
| other | fails with `unknown op <op>` |

## Tests

- `crates/kiln_parse/tests/host_calls.rs`: the interface as a client
  uses it, with a Rust handler that logs every call it receives
  (visibility, order, no memoization, inputs and outputs, every range
  check, failures, overflow, cancellation, parsing, speculation,
  unevaluated operands, the step charge).
- `crates/kiln_driver/src/host_test.rs` (the test handler's ops) and
  `crates/kiln_driver/tests/host_calls.rs`: through the driver (a
  run-time call, no flag, the refused artifacts).
- `tests/conformance/ext.kiln.host_call/`: the builtin through the test
  handler, and `host-call-absent.cpp` without one (both compilers).
