# WP-079 progress (codegen for classes)

Updated: 2026-10-30 18:40 EDT

## State
- `cargo check -p kiln_codegen` builds (dead-code warnings only).
- class.rs: structor variants (RAUW for discardable C1, alias otherwise;
  WP-080 errors for polymorphic / virtual bases), ctor body (bases, fields,
  memcpyizer runs), dtor body (BaseOfThis/Field cleanups), `call_dtor`,
  `emit_construct`, `emit_member_call`, `push_temporary_cleanup`.
- expr.rs: `field_lvalue` handles reference members via `field_lvalue_init`.
- cleanup.rs: `ArrayDtor { addr: Addr, record }` via `array_destroy_at`.

## Next step
Wire expr.rs arms: `CXXThisExpr`, construct exprs in `agg_visit` (call
`emit_construct`), `CXXBindTemporaryExpr`, `CXXMemberCallExpr` (scalar and
agg via `emit_member_call`), `ExprWithCleanups` as a scope,
DerivedToBase casts, `CXXDefaultInitExpr`; then `emit_var_decl` dtor
cleanups / NRVO; then tests.
