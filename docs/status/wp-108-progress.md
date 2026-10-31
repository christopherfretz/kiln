# WP-108 progress

1. [x] Member-pointer RTTI + noexcept function-pointer flag (vtable.rs
   `pointee_rtti`, `member_pointer_rtti`); corpus `virtual_rtti_memptr` exact
   on both triples.
2. [x] `::delete` of a polymorphic class (new_delete.rs; corpus `virtual_global_delete`
   exact both triples); sema diagnostic for member-pointer
   conversion via a virtual base in static_cast/C-style cast (ref_cast.rs
   `static_mp_upcast`; conformance expr.static.cast/member-pointer-virtual-base).
3. [not done] `weak_odr` vtables/RTTI for explicit instantiation definitions:
   larger than a linkage switch. Codegen emits nothing for
   `template struct X<int>;` (sema instantiates the members, codegen only
   emits used functions, linkonce_odr). clang emits every member weak_odr,
   the vtable/RTTI weak_odr, and the destructor as a D5 COMDAT with a
   weak_odr D1 alias; `extern template` makes the vtable `external` and
   suppresses member bodies. Needs: a codegen walk of explicit
   instantiation definitions (members in clang's order), weak_odr in
   `set_def_linkage` / `vtable_linkage` / `rtti_linkage`, the C5/D5
   COMDAT structor form, extern-template suppression. Documented as a
   gap in spec/codegen.md §7a.
4. [x] Corpus `virtual_base_dyncast_hint` (`-1` hint) exact both triples.
   [blocked] Inheriting ctors with vbases: inheriting constructors are not
   implemented at all (sema rejects `using B::B;` with
   sema_using_decl_not_base_class; codegen has no CI1/CI2 support), so the
   spec claim "implemented, untested" was wrong; needs its own WP.

Comparison helper: build `kiln` then diff against clang++-19 through
`opt-19 -passes=verify` (or `KILN_CASE=<stem> cargo test -p kiln_codegen
--test ir_vs_clang`).
