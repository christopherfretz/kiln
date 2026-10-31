# WP-080 progress (codegen for virtual functions)

## Matches clang 19 (both triples, in `tests/codegen/`)
- `virtual_single.cpp`: vtables, class/si RTTI, ctor/dtor vptr stores,
  virtual calls, D0.
- `virtual_key.cpp`: strong vtable (key fn defined here), external vtable
  (key fn elsewhere), abstract class (pure slot, D0 = `llvm.trap`),
  devirtualization (final class / final method / known object), virtual
  `delete` through the D0 slot, devirtualized `delete` (D1 + sized delete).
- `virtual_mi.cpp`: MI secondary groups, `_ZThn` thunks for methods and
  D1/D0, delete via a secondary base.
- `virtual_thunk_args.cpp`: thunks forwarding ref/scalar/short args and an
  sret return.
- `virtual_covariant.cpp`: `_ZTch` return-adjusting thunks (pointer with
  null check + phi; reference).
- ir_vs_clang: 138/138 exact.

## Landed
- mangle.rs: `mangle_class_special`, `mangle_rtti`, `mangle_ctor_vtable`,
  `mangle_thunk`.
- vtable.rs: vtable globals/deferred emission/key-function linkage,
  initializer, RTTI, address points, vptr walk, skip-vptr-init,
  slot loads, devirtualization, thunks (`thunk_fn` declares at vtable
  build; `emit_thunks` defines after the method / D1 / D0, this- and
  return-adjusting; `thunk_attrs` = IsThunk attribute rules). VTT/vbase
  paths are error stubs.
- func.rs: `gen_code_or_thunk`, `emit_trap_unreachable`; virtual call args
  are emitted before the slot load.
- class.rs: vptr stores (ctor after bases, dtor unless skippable), D0
  (`emit_deleting_dtor_body`), virtual/devirtualized member calls.
- new_delete.rs: `emit_object_delete` (dtor call before delete; virtual
  delete via D0 slot; `::delete` of a virtual class still errors).
- sema: `check_destructor_delete` fills `cold.dtor_delete` for virtual dtors.
- module.rs: virtual methods `unnamed_addr`.

## Not yet
- RTTI for pointer types, typeid,
  dynamic_cast, ptr-to-virtual-member (memptr.rs:120), VTT/virtual bases,
  `expect: run` tests, spec text.

## Next step
RTTI for pointer types + `typeid` + `dynamic_cast` (incl. `void*`): scratch
case, compare with clang (`__dynamic_cast` decl, `_ZTIPK...` pointer
type_info). Then ptr-to-virtual-member (memptr.rs:120), then VTT/virtual
bases (`_ZTT`, `_ZTC`, `_ZTv` thunks, vbase offsets, C1-only vbase ctor).
Gotcha: corpus cases must be noexcept throughout (no EH before WP-100).
