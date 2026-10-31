# WP-080 progress (codegen for virtual functions)

## State
- `crates/kiln_abi/src/mangle.rs`: `mangle_class_special`, `mangle_rtti`,
  `mangle_ctor_vtable`, `mangle_thunk` (call-offset encoding).
- `crates/kiln_codegen/src/vtable.rs` (new): vtable globals + deferred
  emission (clang `getAddrOfVTable`/`EmitDeferredVTables`), key-function
  vtables after `emit_members`, initializer (offsets, RTTI, pure/deleted,
  structor slots), RTTI (`__class/si/vmi_class_type_info`, `_ZTS`,
  external fundamental descriptors), address points with `inrange`,
  `getVTablePointers` walk, `CanSkipVTablePointerInitialization`, slot
  loads. VTT/vbase/thunk paths are stubs reporting errors.
- `module.rs`: Cg fields, `run()`/`emit_deferred` ordering wired.

## Not yet wired
- class.rs: vptr stores in ctor/dtor bodies, D0, virtual calls,
  devirtualization; `check_structor` still rejects polymorphic classes.

## Next step
Wire class.rs (see above), build, diff scratch `single.cpp` vs clang 19 on
both triples, add `tests/codegen/virtual_single.cpp`.
