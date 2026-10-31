# WP-119 progress (`decltype(expr)::` nested-name-specifier)

Updated: 2026-10-31

## State: complete, ready for review
- Sema/AST: `AstBuilder::unlinked_alias`, `Sema::act_on_nested_name_decltype`
  (non-dependent -> complete class/enum scope; dependent -> a shared
  dependent-marker alias, NNS `Nns::Type{decltype sugar}`; anything else ->
  `sema_expected_class_or_namespace`), `TemplState::decltype_scope`.
  Substituted `DependentName` keeps its written `typename` keyword (clang
  prints `typename S::type`).
- Parser: a one-slot "decltype scope" annotation (clang's `annot_decltype`),
  cleared on `bump()`, used by decl-specifiers, primary expressions,
  functional casts, id-expressions, template arguments, base specifiers,
  member pointers, and the tentative statement/condition/type-id checks
  (now `&mut self`). Unannotated lookahead skips `decltype(...)::name...`
  syntactically (`decltype_scope_end`, `skip_qualified_rest`).
- Tests: `tests/conformance/expr.prim.id.qual/decltype-nns{,-dependent,
  -errors,-ast}.cpp` (compile-ok x2, strict-diags compile-fail, clang-19 AST
  golden). No decltype xfails existed to flip.
- Specs: "Landed (WP-119)" in `spec/parse.md`, `spec/sema.md`; `unlinked_alias`
  in `spec/ast.md`.

## Evidence
- `cargo xtask ci --quick`: corpus 1496 passed; 0 failed; 15 xfail; 0 xpass
  (1511 tests; main was 1492/1507).
- libc++ probe (`-std=c++26 -stdlib=libc++ -fsyntax-only`, errors before -> after):
  array 87 -> 83, string_view 151 -> 147, span 66 -> 62, utility 23 -> 23.
  New first blocker (array, string_view): `iterator_traits.h:252:37`
  `__i.operator->()` (member access with an operator-function-id, also
  `pointer_traits.h:205/223`). span/utility still stop at range-for
  (`common_comparison_category.h:44`, WP-089).

## Known gaps (pre-existing, not WP-119)
- Unused-value warning located at the name, not the expression start.
- Type-context NNS errors in instantiated aliases reported at the decl name
  (no TypeLocs).
- `decltype(T())::template M<int>::type(7)` without `typename`: kiln says
  "does not refer to a value" where clang says "missing 'typename'".
- Unannotated tentative disambiguation of `decltype(e)::x` is heuristic
  (`unparsed_decltype_scope`); libc++ uses only decl/alias/requires contexts.
