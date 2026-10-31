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
- `virtual_dynamic_cast.cpp`, `virtual_typeid.cpp`: `__dynamic_cast`
  (offset hints 0/-2/-3), `void*` via offset-to-top, reference casts with
  `__cxa_bad_cast`, always-null casts, typeid from the vtable with the
  `__cxa_bad_typeid` deref check, static typeid, pointer/enum/array/
  function/incomplete RTTI.
- `virtual_memptr.cpp`: pointers to virtual member functions.
- `virtual_base_conv.cpp`: derived-to-base through virtual bases
  (vbase offset load, null check, nv tail, final class).
- ir_vs_clang: 146/146 exact.

## Run tests
- `tests/conformance/class.virtual/dispatch-run.cpp` (dispatch, MI,
  covariant, virtual dtor order, ptr-to-virtual-member).
- `tests/conformance/expr.dynamic.cast/run.cpp` (down/cross/failed/void*
  casts, reference casts, typeid names).

## Landed
- mangle.rs: `mangle_class_special`, `mangle_rtti`, `mangle_ctor_vtable`,
  `mangle_thunk`.
- vtable.rs: vtable globals/deferred emission/key-function linkage,
  initializer, RTTI, address points, vptr walk, skip-vptr-init,
  slot loads, devirtualization, thunks (`thunk_fn` declares at vtable
  build; `emit_thunks` defines after the method / D1 / D0, this- and
  return-adjusting; `thunk_attrs` = IsThunk attribute rules). VTT/vbase
  paths are error stubs.
- dyncast.rs: `emit_dynamic_cast`, `emit_typeid` (+ runtime fn helpers).
- vtable.rs RTTI: `pointer_rtti`, enum/array/function type_info,
  `type_rtti_linkage` (incomplete -> internal).
- func.rs: `gen_code_or_thunk`, `emit_trap_unreachable`; virtual call args
  are emitted before the slot load.
- class.rs: vptr stores (ctor after bases, dtor unless skippable), D0
  (`emit_deleting_dtor_body`), virtual/devirtualized member calls.
- new_delete.rs: `emit_object_delete` (dtor call before delete; virtual
  delete via D0 slot; `::delete` of a virtual class still errors).
- sema: `check_destructor_delete` fills `cold.dtor_delete` for virtual dtors.
- module.rs: virtual methods `unnamed_addr`.

## Not yet
- VTT/virtual bases (all of it), member-pointer RTTI,
  noexcept-function-pointer flag, `::delete` of a polymorphic class.

## Spec
- spec/codegen.md §7a, spec/abi.md §8.7.

## Next step
DONE: virtual-base conversions (`base_path` / `base_address`). Was: `base_path_offset`
(class.rs ~1152/1168) must return (virtual step, non-virtual offset);
emit clang `GetVirtualBaseClassOffset` (vtable load, non-inbounds
`gep i8 vt, vbase_offset_offset`, `load i64 align 8`, add nv offset,
`gep inbounds i8`), with null check for pointers. Then structors:
C1 != C2 / D1 != D2, VTT param (`needs_vtt_param` stub in vtable.rs),
`_ZTT`/`_ZTC` emission, VTT-loaded vptrs, `_ZTv` thunks.
Gotcha: corpus cases must be noexcept unless they need a throwing runtime
call with no cleanups (no EH before WP-100).
