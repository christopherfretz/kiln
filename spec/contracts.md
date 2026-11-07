# spec: contracts (P2900R14, `pre` / `post` / `contract_assert`)

Standard: [basic.contract] ([basic.contract.general],
[basic.contract.eval], [basic.contract.handler]), [dcl.contract.func],
[dcl.contract.res], [stmt.contract.assert], [basic.scope.contract],
[expr.prim.id.unqual]/7 and [expr.prim.this]/2 (const-ness in a
predicate), [dcl.fct.def.coroutine]/6, [dcl.fct.def.coroutine]/7 (the return object), [temp.expl.spec]/12,
[intro.defs] (contract violation, contract-violation handler), [cpp.predefined]
(`__cpp_contracts`). Draft: the Croydon DIS (C++26 as balloted;
https://eel.is/c++draft/). Papers: P2900R14 with the Kona amendments.
Status: parsed, analysed and dumped (WP-706); evaluated in the Oven and
in codegen, with kiln's violation handler where the standard library
has none (WP-707, §6); coroutine postconditions and the remaining sema
rules (WP-755). This file is the
contract; spec/ast.md §4 and §6, spec/driver.md (Options) and
spec/sema-templates.md (substitution) point here.

The oracle is GCC 16.1 (`oracle: gcc` tests, spec/harness.md "GCC
oracle"). It implements P2900 at `-std=c++26` with no extra flag; clang
19 and 22 do not, so every contracts test is `skip(clang)`. Where GCC
disagrees with the wording kiln follows the wording and §9 lists the
divergence.

## 1. Scope

In: function contract specifiers (`pre`, `post`, with or without a
result name) on every kind of function declarator (namespace scope,
block scope, members, out-of-line member definitions, friends, function
templates, lambdas, coroutines), the `contract_assert` statement, the
const-ness rules of a predicate, the redeclaration rules, template
instantiation of the specifiers, the ast-dump, the evaluation-semantic
driver knob and `__cpp_contracts` (`202502L` at C++26; absent before;
`kiln_pp/src/predef.rs` KILN_FEATURES).

Out (WP-707): evaluation in the Oven and in codegen, the violation
handler and `<contracts>`. Out (later): P3598 (splices in contract
predicates), contract labels, per-assertion semantics.

## 2. Lexing

`contract_assert` is a keyword at C++26 (`kiln_pp/src/builtin.rs`; at
`-fclang-compat=22`, as at 19, `__is_identifier(contract_assert)` is 0
at C++26 and 1 before, and it is an ordinary identifier before C++26).
`pre` and `post` are identifiers with a special meaning in one place
only: `pre` or `post` followed by `(` or by `[[`, in the position of §3
(`Parser::at_contract_spec`, `kiln_parse/src/contracts.rs`). Anywhere
else they are plain names, also at C++26.

## 3. Grammar and placement (kiln_parse `contracts.rs`)

```
function-contract-specifier-seq: function-contract-specifier+
function-contract-specifier:
    pre attribute-specifier-seq? ( conditional-expression )
    post attribute-specifier-seq? ( result-name-introducer? conditional-expression )
result-name-introducer: attributed-identifier :
assertion-statement:
    contract_assert attribute-specifier-seq? ( conditional-expression ) ;
```

- **Position** ([dcl.decl.general]): after the trailing return type and
  the trailing requires-clause of a function declarator; in a member
  declaration also after the virt-specifiers, before a pure-specifier,
  `= default`, `= delete` or the body; in a lambda after the trailing
  return type and requires-clause, before the body. A seq on a
  declarator that is not a function declarator is not recognised (the
  tokens are then a syntax error, as in GCC).
- **Late parsing.** The seq is captured as tokens where it is written
  and replayed once the function is declared, so the result name's type
  is the declared return type ([dcl.contract.res]/1). A member's seq is
  a complete-class context ([class.mem.general]/7) and is replayed with
  the default arguments and noexcept-specifier after the class. A
  definition whose return type is deduced replays it after the body
  ([dcl.contract.res]/2), so `r` has the deduced type.
- **Attributes** after `pre`, `post`, `contract_assert` or on the
  result name are parsed and ignored with the warning
  `parse_contract_attribute_ignored` (group `-Wignored-attributes`), as
  GCC does.
- **Predicate**: a *conditional-expression*, so an unparenthesised
  assignment or comma is a syntax error ([dcl.contract.func]/1).
- **Where a seq may appear** ([dcl.contract.func]/3-6):
  - only the first declaration introduces the assertions; a later
    declaration repeats the same seq (§5) or has none;
  - not on a deleted function or one defaulted on its first
    declaration ([dcl.contract.func]/6);
  - not on a virtual function ([dcl.contract.func]/6 of the DIS; see §9);
  - coroutines may carry one, but a postcondition may not odr-use a
    non-reference parameter ([dcl.fct.def.coroutine]/6).

## 4. AST (kiln_ast)

- **`StmtKind::ContractAssertStmt`** is every contract assertion:
  `contract_assert` in a body, and each `pre`/`post` of a function.
  Builder `contract_stmt(kind, cond, result, loc)`; `a` = the predicate
  (contextually converted to `bool`, a full-expression), `b` = the
  postcondition's `ContractResultName` decl (0: none), `loc` = the
  keyword, `aux` bits 0-1 = `ContractKind` (`Assert` 0, `Pre` 1, `Post`
  2; `ContractKind::MASK`), bits 2-3 = the evaluation-semantic slot
  (`SEMANTIC_SHIFT`; always 0 = the translation unit's semantic until a
  per-assertion semantic exists, §6). Accessors on `StmtArena`:
  `contract_kind`, `contract_cond`, `contract_result_name`,
  `contract_semantic_slot`.
- **`decls.cold.contracts: ColdMap<DeclId, ListId>`**: the
  `ContractAssertStmt`s of the function contract specifiers, in written
  order, keyed by the declaration that wrote them (a redeclaration that
  repeats a seq gets its own list; one that omits it gets none).
  `Ast::function_contracts(f)` is the list on `f`, else on
  `Ast::contract_first(f)`: its first declaration, except for an explicit
  specialization, whose redeclaration chain starts with the implicit
  declaration made by deduction or class instantiation; there it is the
  first *written* declaration, because the template's seq has no effect
  on an explicit specialization ([temp.expl.spec]/12, P2900R14 §3.4.7),
  which has only its own (WP-755). `contract_param_aliases` uses the
  same declaration. A function template's seq is on its pattern; an
  instantiated specialization gets its own substituted list. The
  `has_contracts` function flag is set on every declaration that carries
  a list.
- **`DeclKind::ContractResultName`**: the result binding of `post(r :
  ...)`, of the function's return type as written (a pattern's is
  `const T`, not the canonical `type-parameter-0-0`), named as a `const` lvalue
  ([dcl.contract.res]/1); its scope is that postcondition only
  ([basic.scope.contract]); its context is the function.
- **`Ast::contract_semantic`**: the translation unit's
  `ContractSemantic` (`Ignore`, `Observe`, `Enforce` default,
  `QuickEnforce`), set from `ParseOptions::contract_semantic`.
  `ContractSemantic::checks()` is false only for `Ignore`.

## 5. Sema rules (kiln_sema `contracts.rs`)

- **Predicate**: contextually converted to `bool`
  ([dcl.contract.func]/2, [stmt.contract.assert]/1) and a
  potentially-evaluated full-expression; a predicate that does not
  convert is the usual conversion error. The assertion is analysed
  between `act_on_start_contract` and `act_on_end_contract` (function
  specifiers) or `act_on_start_contract_assert` (statements).
- **Implicit const-ness** ([expr.prim.id.unqual]/7): an id-expression in
  the predicate that names a variable (parameter, local, static local,
  global, member of an anonymous union), a structured binding or a
  reference template parameter declared outside the assertion has its
  type made `const` (a reference names a `const` object; a pointer
  becomes a const pointer, the pointee is unchanged). A result name is
  always `const`. `this` is a pointer to const ([expr.prim.this]/2), so
  every non-static member access through it is const. The rule follows
  the name into lambdas in the predicate: a by-reference capture and an
  implicit capture name the outer entity and stay const, and `this` in
  such a lambda is a pointer to const; only a by-copy capture of a
  `mutable` lambda declared in the predicate is a member of the closure
  and is modifiable. Such a lambda captures the function's parameters
  and the result binding like any local entity of the enclosing
  function ([expr.prim.lambda.capture]/3.3, [basic.pre]/9; explicitly,
  `[=]`, `[&]`, by copy and by reference), and the captures carry the
  same const-ness (WP-755). Watermark: a declaration whose index is below
  `ContractCtx::watermark` was declared outside the assertion.
- **Splices** ([expr.prim.splice]/2.5, P3598R0, with `-freflection`):
  a splice-expression `[: r :]` in the predicate that designates such a
  variable or structured binding is built through the same decl
  reference (`act_on_splice_expr` -> `build_decl_ref`) and gets the same
  const type, also when `r` is a template argument (on the
  instantiation); `this->[: ^^C::m :]` is const through `this`. A splice
  designating an object (`[: std::meta::object_of(^^v) :]`, /2.3) keeps
  the object's type: `splice_object_expr` builds it with the contract
  context cleared, so it is the visible way to modify a variable from a
  predicate, as the paper intends (WP-765).
- **Implicit captures only for contract assertions**
  ([expr.prim.lambda.closure]/10): a lambda whose implicit capture of a
  local entity (a variable, a structured binding, `*this`) has every
  potential reference in its call operator's function contract
  assertions or in assertion-statements of its body is ill-formed:
  `sema_contract_only_implicit_capture` ("'i' is not implicitly captured
  by a contract assertion", GCC's words; for a variable with the note
  "'i' declared here") at the first such reference, once per lambda and
  entity, checked when the lambda ends (`check_contract_only_captures`).
  A reference from a lambda nested in the assertion counts for every
  enclosing lambda the capture passes through (Example 6, f7); one
  outside any assertion, before or after, makes the capture ordinary.
  Each new implicit capture made inside an assertion is recorded per
  lambda (`LambdaScope::contract_caps`, `note_contract_capture_use` in
  `lambda/capture.rs`) and dropped at a reference outside the
  assertions. A reference that is not an odr-use (a constant's value)
  captures nothing and is not diagnosed. In a template the check runs
  on the definition (the pattern's lambda captures as written) and an
  instantiation does not report it again; a nested lambda reported for an entity is not
  reported again for its enclosing lambda (WP-765).
- **Modification**: assigning to (or incrementing) a name that is const
  only by the rule above reports `sema_contract_const_modify` ("cannot
  modify 'x' in a contract predicate: it is implicitly const"), for a
  plain name and for a member accessed through `this`; anything else
  that is const keeps the ordinary diagnostic. Calling a non-const
  member function through `this` is the ordinary "'this' argument has
  type 'const X'" error.
- **Result name** ([dcl.contract.res]): `post(r : ...)` on a function
  returning cv `void` is `sema_contract_result_void`; on a non-template
  function with an undeduced placeholder return type it may appear only
  on the definition (`sema_contract_result_deduced`); `r` in a `pre` is
  not declared (it is an ordinary unqualified name there).
- **Postcondition parameters** ([dcl.contract.func]/7): a non-reference
  parameter odr-used by a postcondition is `const` on every declaration
  of the function, also the ones without a seq; an array or function
  parameter is adjusted to a pointer and is never const in this sense
  (`sema_contract_post_nonconst_param` at the parameter, once per
  parameter; the parameter stays valid, so the body and a constant
  evaluation are unaffected). In a template (a function template, a
  member or a friend of a class template) a parameter of non-dependent
  type is checked on the definition, once (its instantiations do not
  report it again: `post_param_pattern_locs`); one of dependent type at
  instantiation. A friend redeclaration is checked too, with or without
  a seq (`check_friend_without_contracts` in `kiln_parse/src/class.rs`);
  a non-dependent friend of a class template without a seq is checked
  on the definition against the namespace function it will redeclare
  (`pattern_friend_prev`, recorded in `friend_pattern_prev`) (WP-765).
- **Coroutines** ([dcl.fct.def.coroutine]/6): when the body turns out
  to be a coroutine, a postcondition that odr-uses a non-reference
  parameter is `sema_contract_coroutine_post_param` (once per
  parameter; not again as `sema_contract_post_nonconst_param`).
- **Redeclarations** ([dcl.contract.func]/4-5): a later declaration
  with a seq whose first declaration had none is
  `sema_contract_redecl_adds`; one with a different seq is
  `sema_contract_redecl_mismatch`, with a note at the first declaration.
  Two seqs are the same when they have the same kinds in the same order,
  a result name on both or neither, and equivalent predicates
  (`exprs_equivalent`, the redeclaration-equivalence of
  [temp.over.link], so renamed parameters and result names match).
- **Deleted, defaulted, virtual**: `sema_contract_deleted_or_defaulted`
  and `sema_contract_virtual` ([dcl.contract.func]/6: "A virtual
  function, a deleted function, or a function defaulted on its first
  declaration shall not have a function-contract-specifier-seq"); the
  seq is then dropped. A function is virtual when its first declaration
  is (declared `virtual` or overriding).
- **Explicit specializations** ([temp.expl.spec]/12,
  [dcl.contract.func]/4-5): the template's seq has no effect on an
  explicit specialization; its own first written declaration introduces
  its assertions, and a later redeclaration that adds a seq or differs
  is `sema_contract_redecl_adds` / `sema_contract_redecl_mismatch`, as
  for any function (`Ast::contract_first`).
- **Templates**: a function template's seq is analysed on the pattern
  (dependent predicates stay dependent) and substituted with the
  function (`templ/subst/contracts.rs`): `pattern_contracts` finds the
  declaration of the pattern that carries the seq, each assertion is
  re-run through `act_on_start_contract`/`act_on_end_contract` with the
  substituted predicate, and `install_instantiated_contracts` records
  the list on the specialization. A member of a class template is
  substituted with its class; a generic lambda's are on its call
  operator pattern.
- **Overloading**: the seq is not part of the function type or
  signature; it does not affect overload resolution, mangling or
  redeclaration matching ([dcl.contract.func]/9).

## 6. Evaluation (WP-707)

### 6.1 Semantics and the knob

The four semantics of [basic.contract.eval]/1: `ignore` (not evaluated;
the predicate is still odr-used), `observe` (checked; a violation calls
the handler and continues), `enforce` (checked; a violation calls the
handler, then contract-terminates), `quick_enforce` (checked; a
violation contract-terminates at once, no handler). The driver flag
`-fcontract-evaluation-semantic=ignore|observe|enforce|quick_enforce`
(GCC's spelling; `quick-enforce` accepted too; default `enforce`,
[basic.contract.eval]/3) sets `Ast::contract_semantic` for the whole
translation unit; any other value is "unsupported argument". A
`ContractAssertStmt` whose `aux` semantic slot is 0 uses it; slots 1-3
are reserved for a per-assertion semantic (a later extension) and are
never written today.

### 6.2 Today (WP-707)

- Every semantic is implemented in codegen (§6.3) and the Oven (§6.4).
  `ignore` emits and evaluates nothing (the predicates are still
  odr-used, so they are instantiated and checked).
- A function without contract assertions pays nothing in either: no
  extra IR, no extra Oven bytecode or per-call work
  (`checking_semantics_cost_nothing_without_contracts`,
  `tests/stress/contracts-constexpr-1e6.cpp`).
- Coroutines (WP-755): the preconditions are checked at entry, before
  the frame is allocated; the postconditions are checked once, when the
  coroutine first returns to its caller (the ramp returns), after
  `get_return_object()`, the initial suspend and, for a coroutine that
  does not suspend initially, its run to the first suspension; the
  result binding denotes the return object (P2900R14 §3.5.2,
  [dcl.fct.def.coroutine]/7). In codegen the check sits in the
  function's return block, which follows `llvm.coro.end`, so the resume
  and destroy clones never reach it. The Oven never evaluates a
  coroutine (a coroutine is not constexpr, [dcl.constexpr]/3,
  [expr.const]/5.23; `kiln_oven/src/compile.rs` traps on the body), so there
  is nothing to check there.
- Contracts on the first declaration of a function and evaluated for a
  later definition name the first declaration's parameters; codegen and
  the Oven alias them to the definition's
  (`Ast::contract_param_aliases`).

### 6.3 Codegen (callee side, as GCC)

- Preconditions are evaluated in order at function entry, after the
  parameters are initialised and before the body (and before a
  constructor's mem-initializers); postconditions at each normal return
  after the result object is initialised and local objects are
  destroyed, with the result name bound to the result object (or the
  returned reference); `contract_assert` where it stands. Exits by an
  exception skip postconditions ([dcl.contract.func]/1).
- A violation is a false predicate or an exception escaping it
  ([basic.contract.eval]/7). The predicate is evaluated in a scope whose
  exception handler records detection mode `evaluation_exception` and
  makes the handler run inside that implicit handler
  ([basic.contract.eval]/12).
- The handler ABI is libstdc++'s, which GCC 16.1 uses: a constant
  `std::contracts::contract_violation` object (fields `_M_version` = 1,
  `_M_assertion_kind` (pre 1, post 2, assert 3), `_M_evaluation_semantic`
  (ignore 1, observe 2, enforce 3, quick_enforce 4), `_M_detection_mode`
  (predicate_false 1, evaluation_exception 2), `_M_comment` = the
  predicate's source text, `_M_src_loc_ptr` = a
  `std::source_location::__impl` for the assertion, `_M_ext` = null) in
  read-only data, passed to the replaceable
  `void handle_contract_violation(const std::contracts::contract_violation&)`
  (`_Z25handle_contract_violationRKNSt9contracts18contract_violationE`;
  the default, which libstdc++ provides, calls
  `invoke_default_contract_violation_handler`). Under `enforce` a normal
  return from the handler contract-terminates with `std::terminate()`
  ([basic.contract.eval]/9, 13); `quick_enforce` calls `std::terminate()`
  directly ([basic.contract.eval]/10). An exception from the handler of
  a function contract assertion behaves as if thrown by the function
  ([basic.contract.eval]/17).
- Code (`kiln_codegen/src/contracts.rs`): `emit_preconditions` runs from
  the function prologue, after the parameters are stored and before
  the body; `emit_postconditions` from the return block, after the
  locals' cleanups, with the result name bound to the return slot (for
  a reference result, the returned reference). A thunk, a delegating
  destructor variant and a lambda's static invoker do not check (the
  function they forward to does). Each check is a branch to a cold
  violation block; the `contract_violation` object (`.contract`, with
  its `.contract.loc` source location `{file, function, line,
  column}`, where the function is clang's `__PRETTY_FUNCTION__` and
  the column is the keyword's) is a private constant, one per
  assertion and detection mode. Under `observe`/`enforce` the predicate
  runs in an implicit `catch (...)` whose handler calls
  `handle_contract_violation` with detection mode
  `evaluation_exception` (`__cxa_begin_catch`/`__cxa_end_catch`
  around it, so `std::current_exception()` is the exception and an
  exception from the handler propagates); under `quick_enforce` an
  escaping exception reaches a terminate scope. `-fno-exceptions`
  drops the implicit handler.
- **The handler (libc++ and libstdc++ < 16).** The selected standard
  library either ships `<contracts>` (libstdc++ 16) or not (libc++ 19
  and 22, libstdc++ 14). kiln decides per compile, in the driver
  (`Toolchain::stdlib_ships_contracts`, a `contracts` file in one of
  the stdlib's include directories), and passes
  `CodegenOptions::contract_default_handler`:
  - Without one, kiln's own `resource/include/contracts` serves (the
    resource directory is searched after the stdlib's, so a stdlib
    header always wins). It declares `std::contracts` with
    libstdc++'s layout and ABI (`assertion_kind`,
    `evaluation_semantic`, `detection_mode`, `contract_violation` with
    `kind()`, `semantic()`, `mode()`, `comment()`, `location()`,
    `is_terminating()`, and
    `invoke_default_contract_violation_handler`), and defines
    `__cpp_lib_contracts` (202502L). A module that checks an
    assertion also gets weak definitions of
    `handle_contract_violation` (calls the default) and of
    `invoke_default_contract_violation_handler` (also when only
    referenced), which flushes `stdout` and prints GCC's message to
    `stderr`:
    `contract violation in function F at FILE:LINE: COMMENT` and
    `[assertion_kind: K, semantic: S, mode: M, terminating: yes|no]`.
    A user's (strong) definition replaces either at link time; no
    runtime library is needed.
  - With one, kiln emits no default: the program links the stdlib's
    handler (libstdc++ 16: with `-lstdc++exp`, as under GCC).

  The alternatives were rejected: libstdc++'s handler cannot be
  linked into a libc++ program (a second C++ runtime), and
  `std::terminate` only would break `observe` and the replaceable
  handler ([basic.contract.handler]).

### 6.4 The Oven

- In a manifestly constant-evaluated context
  ([basic.contract.eval]/7-8): under `ignore` the assertion is skipped;
  under a checking semantic the predicate is evaluated, and a false
  predicate, an exception, or a predicate that is not a core constant
  expression is a violation. Under `enforce` and `quick_enforce` it is
  an error: "contract predicate is false in constant expression" (GCC's
  wording; "contract condition is not constant" for a non-constant
  predicate), with the call-stack notes. Under `observe` it is a
  warning with the same text and evaluation continues.
- In a context that is not manifestly constant-evaluated (a tentative
  constant initialisation, `__builtin_constant_p`), a violation makes
  the evaluation non-constant and the code falls back to run time,
  where codegen's checks apply; nothing is diagnosed (GCC agrees).
- The Oven evaluates predicates with the same step, depth and memory
  budgets as the rest of the evaluation; a predicate's side effects are
  allowed only on objects whose lifetime began in the evaluation (the
  usual rule). A postcondition reads the result object in place.
- Code: the predicates are compiled once, into the function's own
  bytecode (`kiln_oven/src/compile.rs` `scan_contracts`,
  `contract_check`, `check_posts`): preconditions after the parameter
  prologue, postconditions at each normal return after the locals are
  destroyed, with the result register (or, for a reference or class
  result, its address) bound to the result name. A false predicate
  executes `Op::ContractFail`; under `observe` the predicate runs as a
  trial (`Op::ContractTrial`) so a non-constant predicate can be
  reported and evaluation go on. Under `ignore` nothing is compiled.
- A variable's initializer that is not `constexpr`/`constinit` is
  first tried with every assertion ignored ([expr.const.init]/4.1);
  only if that is constant is it evaluated again with the semantic
  (`Oven::check_contract_init`; sema calls it only in a translation
  unit that has contract assertions).

## 7. ast-dump

`-fdump-ast` (spec/ast-dump.md) prints a function's specifiers as
children of its `FunctionDecl` (or the lambda's `LambdaExpr`), after
the `ParmVarDecl`s and before the body, in written order:

```
|-ContractAssertStmt pre
| `-BinaryOperator 'bool' '>'
|   |-ImplicitCastExpr 'int' <LValueToRValue>
|   | `-DeclRefExpr 'const int' lvalue ParmVar 'a' 'int'
|   `-IntegerLiteral 'int' 0
|-ContractAssertStmt post
| |-ContractResultNameDecl referenced r
| `-BinaryOperator 'bool' '>'
|   ...DeclRefExpr 'const int' lvalue ContractResultName 'r'
```

A `contract_assert` is `ContractAssertStmt assert` in its compound
statement. A DeclRefExpr in a predicate shows the const-ified type and,
after the decl, the declared type. clang has no such nodes, so the
`ast` tests are goldens (`oracle: none`).

## 8. Diagnostics

| id | text |
|---|---|
| `parse_contract_attribute_ignored` | warning: attributes are ignored on 'contract_assert' / function contract specifiers |
| `sema_contract_const_modify` | cannot modify 'x' in a contract predicate: it is implicitly const |
| `sema_contract_coroutine_post_param` | a postcondition of a coroutine cannot odr-use the non-reference parameter 'x' |
| `sema_contract_deleted_or_defaulted` | deleted/defaulted function cannot have function contract specifiers |
| `sema_contract_only_implicit_capture` | 'x' is not implicitly captured by a contract assertion (note: 'x' declared here) |
| `sema_contract_post_nonconst_param` | a value parameter used in a postcondition must be const |
| `sema_contract_redecl_adds` | declaration adds function contract specifiers to 'f' (note: previous declaration) |
| `sema_contract_redecl_mismatch` | function contract specifiers of 'f' do not match the previous declaration (note) |
| `sema_contract_result_deduced` | postcondition with a result name on a function with a deduced return type must appear on its definition |
| `sema_contract_result_void` | function does not return a value to test |
| `sema_contract_virtual` | function contract specifiers cannot be added to virtual functions |

Oven (§6.4): `oven_contract_predicate_false` /
`oven_contract_condition_not_constant` (errors, enforce and
quick_enforce) and their `_observe` warnings, as notes of the
evaluation's failure with the call stack.
Driver: `unsupported argument 'v' to option '-fcontract-evaluation-semantic='`.

## 9. Known gaps, divergences and open questions

Divergences from GCC 16.1 (kiln follows the wording; the tests have no
oracle):

- GCC accepts a seq on a deleted or defaulted function;
  [dcl.contract.func]/6 forbids it (`p6-deleted-defaulted.cpp`).
- GCC does not const-ify names inside a lambda in a predicate, makes an
  explicit by-copy capture `[n]` const but not an implicit `[=]` one,
  and rejects naming a parameter in such a lambda; kiln applies
  [expr.prim.id.unqual]/7 throughout (`const-lambda.cpp`,
  `mutable-copy-capture.cpp`).
- GCC accepts a coroutine postcondition that odr-uses a value
  parameter; [dcl.fct.def.coroutine]/6 forbids it.
- GCC reports an exception escaping a predicate as detection mode
  `predicate_false`; [basic.contract.eval]/7 makes it
  `evaluation_exception`, which kiln reports
  (`run-evaluation-exception.cpp`). Both run the handler inside the
  implicit handler.
- The handler's function name is clang's `__PRETTY_FUNCTION__`
  (`int f(const int)`, `[T = int]`, `auto main()::(lambda)::operator()(int)
  const`), not GCC's (`int f(int)`, `[with T = int]`,
  `main()::<lambda(int)>`); the comment is the predicate's source text
  as written (with macro names when the predicate is inside a macro
  argument, else the outermost expansion's text), where GCC
  pretty-prints the expression.
- kiln's default handler flushes `stdout` first; libstdc++'s writes
  through `std::cerr`, which is tied to `std::cout`, so the visible
  order is the same for `printf`-only programs.

Divergences recorded by WP-755 (`oracle: none` unless noted):

- GCC rejects naming a parameter (or the result binding) in a lambda in
  a predicate; kiln captures it ([expr.prim.lambda.capture]/3.3: a
  parameter is a local entity of the function, and the predicate is in
  its scope; `lambda-capture-params.cpp`, `run-lambda-capture.cpp`).
- GCC does not diagnose an explicit specialization whose redeclaration
  adds a seq or has a different one, and drops the seq of the
  specialization's first declaration when the definition omits it;
  kiln applies [dcl.contract.func]/4-5 to the specialization's own
  declarations and checks the first one's seq at every call
  (`p4-explicit-spec.cpp`, `run-explicit-spec.cpp`).
- GCC reports the postcondition-parameter const rule at the predicate's
  use of the parameter; kiln reports it at the parameter of each
  offending declaration, including a friend redeclaration without a seq
  and the earlier declaration when a friend redeclaration adds the seq
  (`p7-friend-redecl.cpp` is compared with GCC on the verdict only, as
  every `oracle: gcc` compile-fail test is).

Divergences recorded by WP-765:

- P3598R0 ([expr.prim.splice]/2.5): GCC 16.1 const-ifies a splice of
  an object (`[: std::meta::object_of(^^g) :]`, /2.3) as well; the
  wording const-ifies only a splice designating a variable or a
  structured binding, so kiln lets the object splice modify `g`
  (`expr.prim.splice/contract-constify-object.cpp`, no oracle). GCC also
  misses the const-ification of a splice whose reflection is a template
  argument (`t<^^g>`); kiln reports it on the instantiation
  (`contract-constify.cpp` is compared on the verdict only).
- [expr.prim.lambda.closure]/10: GCC 16.1 does not diagnose a capture
  that reaches the assertion only through a nested lambda (Example 6,
  f7) or an implicit `this` capture, and diagnoses a constant whose use
  is not an odr-use (it captures nothing);
  `p10-contract-only-capture-kiln.cpp` has no oracle,
  `p10-contract-only-capture.cpp` (the rest of Example 6) is compared
  with GCC.
- [dcl.contract.func]/7 in templates: GCC checks a friend
  redeclaration without a seq only when the class is instantiated, never
  checks a friend *definition* (`h`), and never checks a dependent
  friend; kiln checks every non-dependent case on the definition,
  once for the pattern (`p7-template-definition.cpp`, no oracle).
- A violation in an explicit specialization of a member of a class
  template names the specialization as clang 22's
  `__PRETTY_FUNCTION__` does: `int C<long>::m(long) [T = long]` (the
  specialization's own parameter types, with clang's bracket for the
  enclosing class's arguments); GCC prints
  `int C<T>::m(T) [with T = long int]`
  (`basic.contract.eval/run-site-name-explicit-spec.cpp`,
  `dcl.fct.def.general/predefined-member-explicit-spec.cpp` compared
  with clang). The name comes from sema's `predefined_function_names`
  (`Sema::function_pattern` stops at a member explicit
  specialization), shared with `__PRETTY_FUNCTION__`.

Gaps (follow-ups):

- `-fcontract-evaluation-semantic=` has no per-assertion form;
  `aux` slots 1-3 are reserved.
- A friend of a class template whose parameter type is dependent
  (`friend int m(T a);` redeclaring `int m(const int a) post(r: r == a)`)
  is not checked against the postcondition-parameter rule on the
  instantiation either (GCC 16.1 does not check it).
- `^^p` / `[: ^^p :]` of a local across an intervening lambda
  ([expr.prim.splice]/2.1.4, [expr.reflect]) is not diagnosed (also
  outside contracts; `kiln_sema` reflection splice).

Decisions:

- **Virtual functions (WP-755).** The DIS (P2900R14 as balloted, whose
  revision 14 "made ill-formed pre and post on virtual functions,
  again") says in [dcl.contract.func]/6: "A virtual function
  ([class.virtual]), a deleted function ([dcl.fct.def.delete]), or a
  function defaulted on its first declaration ([dcl.fct.def.default])
  shall not have a function-contract-specifier-seq." kiln keeps
  `sema_contract_virtual`, now on the wording rather than on GCC (which
  agrees; `virtual.cpp`, `oracle: gcc`). The working draft at eel.is no
  longer has the virtual clause: P3097 (contracts on virtual functions)
  was adopted for C++29; implementing it is a follow-up behind a
  language-mode check, not a C++26 change.
- **P3097 (contracts on virtual functions) is not implemented
  (WP-765).** It is C++29 work and not in the Croydon DIS, which kiln's
  C++26 mode targets. It removes "A virtual function" from
  [dcl.contract.func]/6 and, as the paper proposes, checks on a
  virtual function call both the statically chosen function's and the
  final overrider's seqs (the caller-facing and callee-facing
  contracts; an overrider's seq is independent of the overridden
  one's). Implementing it
  needs a language mode after C++26 (kiln has none yet), a gate on
  `sema_contract_virtual` keyed on it, the overriding rules in
  `contracts.rs` and the caller-side evaluation in codegen and the Oven
  (§6.3 evaluates on the callee side only). Until then a seq on a
  virtual function is `sema_contract_virtual` in every mode.
- **Coroutines (WP-755).** The DIS permits a seq on a coroutine (only
  the postcondition-parameter rule, [dcl.fct.def.coroutine]/6, applies).
  The postconditions are evaluated when the coroutine first returns to
  its caller, with the result binding denoting the return object, the
  prvalue result object initialised by `get_return_object()`
  ([dcl.contract.res]/1, [dcl.fct.def.coroutine]/7, P2900R14 §3.5.2),
  never on a resume (§6.2); GCC 16.1 agrees
  (`run-coroutine-post-*.cpp`, `oracle: gcc`). A coroutine is never
  constant-evaluated, so the Oven has no coroutine case.
- **Explicit specializations (WP-755).** [temp.expl.spec]/12 (P2900R14
  §3.4.7): an explicit specialization has its own seq or none; the
  template's does not apply. kiln's redeclaration chain of an explicit
  specialization starts at the implicit declaration deduction or class
  instantiation made, so the contract rules start at the first
  *written* declaration (`Ast::contract_first`).
- **Error recovery.** The postcondition-parameter const rule does not
  invalidate the parameter: a body that uses it is still analysed and a
  constant evaluation under `ignore` does not report a spurious
  "control reached end of constexpr function"
  (`ignore-error-recovery.cpp`, `oracle: gcc`).
- **libc++.** kiln's resource `<contracts>` and weak default handler
  (§6.3).

## 10. Tests

- `tests/conformance/dcl.contract.func/`: declarations, placement and
  late parsing (`declarations.cpp`), attributes, redeclarations
  (`p4-*`), deleted/defaulted (`p6-*`), virtual, the postcondition
  parameter rule (`p7-*`), predicate conversion, `run-ignore.cpp`, and
  the dump goldens `dump-function.cpp`, `dump-lambda-template.cpp`;
  `p4-explicit-spec.cpp`, `p7-friend-redecl.cpp`,
  `coroutine-post-param.cpp` (WP-755); `p7-template-definition.cpp`
  (WP-765).
- `tests/conformance/expr.prim.splice/contract-constify*.cpp` (WP-765,
  P3598R0): a write through a splice, the const overload chosen through
  one (compile-fail and run, compared with GCC), and the object splice
  that stays modifiable (no oracle).
- `tests/conformance/expr.prim.lambda.closure/p10-contract-only-capture*.cpp`
  (WP-765): Example 6 compared with GCC, and the nested, `this`,
  template and non-odr-use cases without an oracle.
- `tests/conformance/dcl.contract.res/`: result names, `void`, deduced
  return types, a result name in `pre`, modifying the result.
- `tests/conformance/stmt.contract.assert/`: the statement, syntax,
  the keyword at `-fclang-compat=22` and before C++26, `run-ignore.cpp`,
  `dump-assert.cpp`, `contract-assert.cpp` (enforce, run).
- `tests/conformance/basic.contract/`: const-ness of parameters,
  locals, globals, `this` and in lambdas; the Oven under `ignore`,
  `enforce` and `observe`.
- `tests/conformance/cpp.predefined/contracts-macro*.cpp`:
  `__cpp_contracts`.
- `tests/conformance/basic.contract.eval/`: the Oven under each
  semantic, manifestly (`const-*`, `not-constant-*`) and tentatively
  (`tentative-*`, `not-manifest*`) constant-evaluated; and `run` tests
  compared with GCC 16.1: `run-observe.cpp` (kind, semantic, mode,
  comment, line; also under libc++, `run-observe-libcxx.cpp`),
  `run-enforce.cpp`, `run-quick-enforce.cpp` (replaced handlers and a
  terminate handler); without an oracle (GCC links its default from
  `-lstdc++exp`, or diverges): `run-evaluation-exception.cpp`,
  `run-default-handler.cpp`, `run-invoke-default.cpp`. WP-755:
  `run-coroutine-post-{observe,enforce,quick-enforce,ignore}.cpp` (on
  `Inputs/coroutine-post.h`; compared with GCC), `run-lambda-capture.cpp`,
  `run-explicit-spec.cpp` (no oracle), `ignore-error-recovery.cpp`;
  `run-site-name-explicit-spec.cpp` (WP-765, no oracle) and
  `dcl.fct.def.general/predefined-member-explicit-spec.cpp` (clang).
- `crates/kiln_codegen/tests/contracts.rs`: the IR shape (an `enforce`
  golden, `observe`'s detection modes, `quick_enforce`, no default
  handler when the stdlib ships one, no cost without contracts, the
  coroutine pre- and postcondition placement around `llvm.coro.end`);
  `crates/kiln_driver/src/args.rs` unit test `contract_evaluation_semantic`.
- `tests/stress/contracts-many.cpp`: 10,000 functions with `pre`, `post`
  and `contract_assert`; WP-706 measured 5.6 s and 48 MiB (debug kiln,
  `-fsyntax-only`) against 2.8 s and 39 MiB without the contracts,
  linear in the count.
- `tests/stress/contracts-constexpr-1e6.cpp`: a million constant-evaluated
  calls of a function with `pre` and `post`; the Oven budget pins
  bounded steps per call and no memory per call.
- `tests/stress/contracts-lambda-capture-1e5.cpp` (WP-755): the same
  with a capturing lambda in each predicate; the closures are reclaimed
  at once (Oven peak one closure, no allocations).
