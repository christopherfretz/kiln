# spec: contracts (P2900R14, `pre` / `post` / `contract_assert`)

Standard: [basic.contract] ([basic.contract.general],
[basic.contract.eval], [basic.contract.handler]), [dcl.contract.func],
[dcl.contract.res], [stmt.contract.assert], [basic.scope.contract],
[expr.prim.id.unqual]/7 and [expr.prim.this]/2 (const-ness in a
predicate), [dcl.fct.def.coroutine]/6, [intro.defs] (contract
violation, contract-violation handler), [cpp.predefined]
(`__cpp_contracts`). Draft: the Croydon DIS (C++26 as balloted;
https://eel.is/c++draft/). Papers: P2900R14 with the Kona amendments.
Status: parsed, analysed and dumped (WP-706); evaluated in neither the
Oven nor codegen until WP-707 (§6 is its design). This file is the
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
  - not on a virtual function (GCC 16.1 rejects; see §9);
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
  `Ast::function_contracts(f)` is the list on `f`, else on its first
  declaration. A function template's seq is on its pattern; an
  instantiated specialization gets its own substituted list. The
  `has_contracts` function flag is set on every declaration that carries
  a list.
- **`DeclKind::ContractResultName`**: the result binding of `post(r :
  ...)`, of the function's return type, named as a `const` lvalue
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
  and is modifiable. Watermark: a declaration whose index is below
  `ContractCtx::watermark` was declared outside the assertion.
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
  (`sema_contract_post_nonconst_param` at the parameter). Checked at
  instantiation, not on the pattern.
- **Coroutines** ([dcl.fct.def.coroutine]/6): when the body turns out
  to be a coroutine, a postcondition that odr-uses a non-reference
  parameter is `sema_contract_coroutine_post_param`.
- **Redeclarations** ([dcl.contract.func]/4-5): a later declaration
  with a seq whose first declaration had none is
  `sema_contract_redecl_adds`; one with a different seq is
  `sema_contract_redecl_mismatch`, with a note at the first declaration.
  Two seqs are the same when they have the same kinds in the same order,
  a result name on both or neither, and equivalent predicates
  (`exprs_equivalent`, the redeclaration-equivalence of
  [temp.over.link], so renamed parameters and result names match).
- **Deleted, defaulted, virtual**: `sema_contract_deleted_or_defaulted`
  ([dcl.contract.func]/6), `sema_contract_virtual`; the seq is then
  dropped.
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

## 6. Evaluation (design; WP-707 implements)

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

### 6.2 Today (WP-706)

- `ignore`: codegen emits nothing for any assertion; the Oven skips
  them. Programs with contracts compile, link and run.
- Any other semantic: codegen reports `unsupported: contract
  evaluation (WP-707)` at the first assertion of a function definition
  with function contract specifiers, and at each `contract_assert`
  (`kiln_codegen/src/func.rs`, `stmt.rs`); the Oven traps a call of
  such a function, or a `contract_assert`, with the same note
  (`kiln_oven/src/compile.rs`), so a constant expression that reaches
  one is not constant. A parsed contract is never dropped silently.

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
  ([basic.contract.eval]/17). libc++ has no `<contracts>`; WP-707
  decides what a libc++ build links (§9).

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
| `sema_contract_post_nonconst_param` | a value parameter used in a postcondition must be const |
| `sema_contract_redecl_adds` | declaration adds function contract specifiers to 'f' (note: previous declaration) |
| `sema_contract_redecl_mismatch` | function contract specifiers of 'f' do not match the previous declaration (note) |
| `sema_contract_result_deduced` | postcondition with a result name on a function with a deduced return type must appear on its definition |
| `sema_contract_result_void` | function does not return a value to test |
| `sema_contract_virtual` | function contract specifiers cannot be added to virtual functions |

Until WP-707: the note `unsupported: contract evaluation (WP-707)` (§6.2).
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

Gaps (follow-ups):

- A lambda in a function's contract cannot capture that function's
  parameters (the predicate is replayed outside the parameter scope's
  capture machinery).
- An explicit specialization that adds a seq to a template without one
  is not diagnosed (`act_on_function_contracts`, spec_info branch).
- A friend redeclaration without a seq is not checked for the
  postcondition-parameter const rule.
- The pattern of a function template prints its result name as
  `'const type-parameter-0-0'` in the dump (clang's canonical spelling
  of a dependent type), not `'const T'`.
- After a sema error in a predicate, a constexpr function evaluated
  under `ignore` can get a spurious "control reached end of constexpr
  function" note.
- `-fcontract-evaluation-semantic=` has no per-assertion form;
  `aux` slots 1-3 are reserved.

Open questions:

- **Virtual functions.** The draft's [class.virtual] has no rule against
  a seq on a virtual function; GCC 16.1 rejects one and kiln follows
  GCC. Revisit if the DIS or a DR settles it the other way.
- **Coroutines.** The WP-706 brief said a seq on a coroutine is
  ill-formed; the DIS permits one (only the postcondition-parameter
  rule applies), as does GCC, so kiln accepts it.
- **libc++.** libc++ (19, 22) ships no `<contracts>` and no default
  handler; WP-707 must pick between linking libstdc++'s handler, a
  kiln runtime default, or `std::terminate` only.

## 10. Tests

- `tests/conformance/dcl.contract.func/`: declarations, placement and
  late parsing (`declarations.cpp`), attributes, redeclarations
  (`p4-*`), deleted/defaulted (`p6-*`), virtual, the postcondition
  parameter rule (`p7-*`), predicate conversion, `run-ignore.cpp`, and
  the dump goldens `dump-function.cpp`, `dump-lambda-template.cpp`.
- `tests/conformance/dcl.contract.res/`: result names, `void`, deduced
  return types, a result name in `pre`, modifying the result.
- `tests/conformance/stmt.contract.assert/`: the statement, syntax,
  the keyword at `-fclang-compat=22` and before C++26, `run-ignore.cpp`,
  `dump-assert.cpp`; `contract-assert.cpp` stays an xfail (enforce, WP-707).
- `tests/conformance/basic.contract/`: const-ness of parameters,
  locals, globals, `this` and in lambdas; the Oven under `ignore`,
  `enforce` and `observe`.
- `tests/conformance/cpp.predefined/contracts-macro*.cpp`:
  `__cpp_contracts`.
- `crates/kiln_codegen/tests/contracts.rs`: the codegen gate (§6.2);
  `crates/kiln_driver/src/args.rs` unit test `contract_evaluation_semantic`.
- `tests/stress/contracts-many.cpp`: 10,000 functions with `pre`, `post`
  and `contract_assert`; WP-706 measured 5.6 s and 48 MiB (debug kiln,
  `-fsyntax-only`) against 2.8 s and 39 MiB without the contracts,
  linear in the count.
