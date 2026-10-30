# WP-079 progress (codegen for classes)

Updated: 2026-10-30 18:22 EDT

## State
- Does NOT build: `crates/kiln_codegen/src/class.rs` (declared in lib.rs) is
  missing; module.rs/func.rs/cleanup.rs/stmt.rs reference helpers it will
  define (`decl_sig`, `declare_structor`, `emit_global_structor`,
  `emit_structor_def`, `emit_ctor_body`, `emit_dtor_body`, `call_dtor`,
  `destroy_field_of_this`, `this_value`, `byte_offset`, `emit_agg_slot`,
  `emit_ref_binding`).

## Next step
Write class.rs (structor C1/C2 strategy: RAUW for discardable, alias
otherwise; WP-080 error for polymorphic/virtual bases), then fix residual
compile errors, then expr.rs arms (this, construct, member call, bind
temporary, derived-to-base casts).
