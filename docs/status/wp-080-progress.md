# WP-080 progress (codegen for virtual functions)

## Matches clang 19 (both triples, in `tests/codegen/`)
- `virtual_single.cpp`: vtables, class/si RTTI, ctor/dtor vptr stores,
  virtual calls, D0.
- `virtual_key.cpp`: strong vtable (key fn defined here), external vtable
  (key fn elsewhere), abstract class (pure slot, D0 = `llvm.trap`),
  devirtualization (final class / final method / known object), virtual
  `delete` through the D0 slot, devirtualized `delete` (D1 + sized delete).
- ir_vs_clang: 132/132 exact.

## Landed
- mangle.rs: `mangle_class_special`, `mangle_rtti`, `mangle_ctor_vtable`,
  `mangle_thunk`.
- vtable.rs: vtable globals/deferred emission/key-function linkage,
  initializer, RTTI, address points, vptr walk, skip-vptr-init,
  slot loads, devirtualization. VTT/vbase/thunk paths are error stubs.
- class.rs: vptr stores (ctor after bases, dtor unless skippable), D0
  (`emit_deleting_dtor_body`), virtual/devirtualized member calls.
- new_delete.rs: `emit_object_delete` (dtor call before delete; virtual
  delete via D0 slot; `::delete` of a virtual class still errors).
- sema: `check_destructor_delete` fills `cold.dtor_delete` for virtual dtors.
- module.rs: virtual methods `unnamed_addr`.

## Not yet
- Thunks (MI), covariant returns, RTTI for pointer types, typeid,
  dynamic_cast, ptr-to-virtual-member (memptr.rs:120), VTT/virtual bases,
  `expect: run` tests, spec text.

## Next step
MI with thunks: scratch `mi.cpp` (two polymorphic bases, override in the
derived class) -> implement `thunk_for_vtable` in vtable.rs (clang
`CodeGenVTables::maybeEmitThunk` / `EmitThunk` this-adjust only).
Gotcha: corpus cases must be noexcept throughout (no EH before WP-100).
