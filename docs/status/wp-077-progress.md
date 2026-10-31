# WP-077 progress — virtual dispatch, virtual bases, dynamic_cast, typeid

Branch `wp/077-oven-virtual`. Started 2026-10-30 20:45 EDT.

## Design (decided)
- **vptr-in-object**: each polymorphic subobject's offset-0 8 bytes (the
  ABI vptr slot, never a field) hold an Oven value: low 32 bits = DeclId
  of the current dynamic class C, high 32 = offset of this subobject in C.
  Written by `Op::SetVptr [this, class]` in the ctor prologue (after base
  initializers, before member inits) and at dtor entry ([class.cdtor]).
  Uninit vptr => CWG1517 "subexpression not valid in a constant expression".
- Freeze is structural, so vptr bytes are padding; thaw re-writes them for
  every complete object of polymorphic class type.
- `Op::VCall [method, base]`: read vptr at `this`, locate the subobject of C,
  `bridge.final_overrider`, adjust this, enter. Cached per (C, off, method).
- dynamic_cast / typeid through the same vptr reads.

## Status
- [ ] SetVptr + VCall + thaw reconstruction
- [ ] pure virtual / CWG1517 / external-object notes
- [ ] virtual member pointer calls, virtual dtor delete
- [ ] dynamic_cast
- [ ] typeid
- [ ] virtual bases (minimal)
- [ ] tests, bench, spec

## Next step
Implement SetVptr/VCall ops (kbc.rs, vm.rs, compile/class.rs).
