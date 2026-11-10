# Consteval blocks and the plainly constant-evaluated bit (WP-473)

Status: implemented (M6-04). Standard: [dcl.pre] (P3289R1, merged into
the C++26 working draft with P2996R13), [expr.const.core]/2.32,
[expr.const.imm]/2. Code: `crates/kiln_parse/src/consteval_block.rs`,
`crates/kiln_sema/src/consteval_block.rs`, `crates/kiln_sema/src/consteval.rs`
(escalation), `crates/kiln_oven/src/lib.rs` (`EvalContext::plain`,
`Oven::plain`), `crates/kiln_oven/src/vm.rs` (`Op::IsConstEval`).

## 1. Grammar and parse

```
consteval-block-declaration:
    consteval compound-statement
```

- It is a *block-declaration* (namespace scope, block scope) and a
  *member-declaration* (class scope). `consteval` directly followed by
  `{` starts a consteval block; `consteval` followed by anything else is
  the decl-specifier. `if consteval` (`stmt.rs`) is unchanged.
- Below `-std=c++26` the parser reports `parse_consteval_block_cxx26`
  ("consteval blocks are only supported in C++26") at `consteval` and
  still parses the block (error recovery). `-freflection` is not
  required.
- The parser builds the block as P3289R1's equivalence: a consteval
  lambda `[]() -> void consteval { … }` and a call to it, both inside
  `enter/exit_constant_evaluated`, so the body is an immediate function
  context. The result is `DeclKind::ConstevalBlock`, whose payload is the
  `StaticAssert` record with `cond` = that call (no message). It is
  dumped as `ConstevalBlockDecl` with the call as its child
  (`spec/ast-dump.md`).

## 2. Semantics

- **Evaluation point.** The block is evaluated once, when it is
  declared: at namespace and block scope where it appears, at class
  scope when reached in the class body (the class is incomplete there;
  later members are not yet declared). Sema evaluates `call, true` with
  `EvalContext::consteval_block(loc)` (a void prvalue has no value in
  the Oven). Diagnostics are reported once.
- **Errors are errors.** A block that is not a constant expression is
  `sema_consteval_block_non_constant` ("consteval block is not a constant
  expression") with the Oven's notes, never a substitution failure.
  Local variables, `static_assert`, `return;`, and calls to consteval
  and constexpr functions work as in any consteval lambda body.
- **Templates.** In a template definition (or any dependent context) the
  block is not evaluated. It is instantiated with the enclosing entity
  (`templ/subst/decls.rs` `consteval_block_decl`, class members in
  `templ/class.rs`, function bodies in `templ/subst/stmts.rs`) and
  evaluated once per instantiation. A failure in a class template
  instantiation marks the class invalid, as a failed `static_assert`
  does. In a substituted function or lambda body a failure is the
  block's error only: the body, and a lambda holding it, stay valid
  (spec/sema-templates.md §5.5; WP-1257).
- **Never codegen'd.** The declaration produces no IR; the lambda is an
  immediate function.

Tests: `tests/conformance/dcl.pre/consteval-block-{namespace,block,class,template,errors,cxx23,lambda-no-cascade,lambda-next-error}.cpp`, `dcl.pre/run-consteval-block-lambda-template.cpp`.

## 3. The plainly constant-evaluated bit

P3289R1 introduced "plainly constant-evaluated" for the contexts in
which injection (`define_aggregate`, kiln's `__kiln_define_member`) may
mutate. The merged C++26 wording drops the term:
[expr.const.core]/2.32 makes an injection a non-constant expression
unless the evaluation is that of a consteval-block-declaration's
expression. kiln follows the merged wording: the bit is
`EvalContext::plain`, set only by `EvalContext::consteval_block(loc)`,
and true for exactly that evaluation including the functions it calls.

`Oven::plain()` is `run.plain && !run.fold && !mem.in_trial()`. The
extension builtin `__kiln_plainly_constant_evaluated()` (`bool()`,
constexpr, builtin id 2150, `Op::IsConstEval` aux 2) returns it; at run
time it is `false`. Like `__kiln_define_member` it is a documented kiln
extension, not a standard name.

| # | Context | Bit |
|---|---|---|
| 1 | a consteval block's evaluation (namespace, class, block scope), and every function it calls | true |
| 2 | an evaluation nested in a consteval block: a `static_assert`, a constexpr local's initializer | false |
| 3 | a `static_assert` condition | false |
| 4 | the initializer of a `constexpr` variable | false |
| 5 | the initializer of a `constinit` variable | false |
| 6 | a `const` variable's constant initializer | false |
| 7 | a template argument | false |
| 8 | an `if constexpr` condition | false |
| 9 | an array bound | false |
| 10 | an enumerator initializer | false |
| 11 | a noexcept-specifier operand | false |
| 12 | a bit-field width | false |
| 13 | a `case` label | false |
| 14 | an immediate invocation outside a consteval block | false |
| 15 | a constraint (atomic constraint satisfaction) | false |
| 16 | a trial: the operand of `__builtin_constant_p`, `try_fold`, codegen's speculative folds, SFINAE probes, even inside a consteval block | false |
| 17 | a constexpr function evaluated in a constant expression | false |

Every row is one check in `tests/conformance/expr.const/plainly-table.cpp`.

Rules that keep the bit exact:
- **Constexpr locals.** The initializer of a constexpr local variable is
  its own manifestly constant-evaluated expression, but the Oven
  evaluates it inline in the enclosing frame. The compiler brackets it
  with `Op::IsConstEval` aux 4 (save `run.plain` to a register and clear
  it) and aux 8 (restore it), so row 2 holds.
- **Memoization.** A memoized call's result can depend on the bit (a
  callee reads it), so the memo key carries `Oven::plain()` at bit 30
  (`const_context` stays at bit 31).
- **Injection gate.** The VM builds `MetaCtx { plainly: Oven::plain(),
  trial: run.fold || mem.in_trial(), … }` (`vm/meta.rs`). A `Mutating`
  op fails silently in a trial before it reaches the bridge, and
  the member, access, layout and annotation queries do not instantiate a
  class from a trial (`Sema::meta_may_instantiate`, `meta/member.rs`),
  so a trial never mutates
  (`tests/conformance/dcl.pre/consteval-block-trial-never-mutates.cpp`).
  `__kiln_define_member` is a kiln extension and keeps injecting from
  any manifestly constant-evaluated context in the member-specification
  (a `static_assert` too); `ctx.plainly` is the gate for the standard
  `define_aggregate` (WP-470's family).

## 4. Immediate escalation of consteval-only expressions

[expr.const.imm]/2: an expression of consteval-only type is
immediate-escalating. This section specifies only that behaviour. Which
expressions count (the consteval-only *leaves*: a `^^` expression, a
reference to a variable, parameter or binding of consteval-only type),
where they are recorded, the exemptions (immediate context, a
`constexpr` variable's initializer, a constant-initialized variable
whose initializer folds) and the template re-check are WP-472's
representation, specified once in `spec/reflection-repr.md` §6
(`crates/kiln_sema/src/consteval_only.rs`).

A leaf that is not exempt, checked at `finish_immediate_invocations`:

- inside an immediate-escalating function ([expr.const.imm]/3: a
  lambda's call operator, a defaulted special member, or a constexpr
  function that is a templated entity; see `spec/sema.md` §5.10) makes
  that function an immediate function, exactly like an immediate
  invocation. `Sema::escalate_consteval_only` records the reason, so a
  later use of the function as a non-constant expression explains it
  with `note_immediate_function_reason_consteval_only` ("... because
  its body contains an expression of consteval-only type {0}");
- anywhere else is the error `sema_consteval_only_expr`
  ("consteval-only expressions are only allowed in a constant-evaluated
  context"), once per full-expression and never after an error already
  reported in it.

Test: `tests/conformance/expr.const/immediate-escalating-reflect.cpp`.

A plain (non-template) `auto f() { return ^^int; }` is not
immediate-escalating, so it is ill-formed, not consteval. So is a
non-template `constexpr` function with an `info` parameter or return
type (`sema_consteval_only_fn`) and a non-`constexpr` variable of
consteval-only type (`sema_consteval_only_var`), per
`spec/reflection-repr.md` §6.

Gaps: a call whose result has consteval-only type (`refl<int>()` in a
runtime function) is not diagnosed; `^^` in a default argument or a
default member initializer is checked at its own location, not at the
use.
