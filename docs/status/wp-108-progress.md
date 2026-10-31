# WP-108 progress

1. [x] Member-pointer RTTI + noexcept function-pointer flag (vtable.rs
   `pointee_rtti`, `member_pointer_rtti`); corpus `virtual_rtti_memptr` exact
   on both triples.
2. [ ] `::delete` of a polymorphic class; sema diagnostic for member-pointer
   conversion via a virtual base.
3. [ ] `weak_odr` vtables/RTTI for explicit instantiation definitions.
4. [ ] Corpus cases: `-1` dynamic_cast hint; inheriting ctors with vbases.

Comparison helper: build `kiln` then diff against clang++-19 through
`opt-19 -passes=verify` (or `KILN_CASE=<stem> cargo test -p kiln_codegen
--test ir_vs_clang`).
