# WP-114 progress — inheriting constructors end to end

Branch `wp/114-inheriting-ctors`, worktree `kiln/.worktrees/wp-114`.
Spec: `spec/sema-class.md` §4.10 (Landed), `spec/codegen.md` (Gaps),
`spec/oven.md`.

## Done
- Sema (`using.rs`): class-scope using-declarators (not-a-base,
  own-class and repeated-declarator errors, [namespace.udecl]/14 hiding),
  `using B::B;` as `Using` + one `ConstructorUsingShadow` per base
  constructor (including ones the base inherits), shadows hidden by later
  derived constructors (`hidden_shadow`), dependent qualifiers as
  `UnresolvedUsingValue` replayed at class instantiation (libc++
  `using __base::__base;`).
- Overloading: inherited candidates through shadows (access of the
  using-declaration), the [over.match.funcs.general]/8 slice
  (`InhCtorSlice`, no note for copy/move constructors),
  [over.match.best]/2.7 derived-over-inherited tie-break.
- Inheriting constructor (`class/inhctor.rs`): materialized per (class,
  base constructor), constexpr/deleted/explicit/noexcept as clang, defined
  on first odr-use with `CXXInheritedCtorInitExpr`, virtual-base chains
  (`InheritedFromVBase`).
- Oven: inherited ctor-init evaluates the base constructor with the
  inheriting constructor's arguments; clang's notes (non-constexpr
  inherited constructor; constructor inherited through a virtual base).
- Codegen/ABI: `CI1`/`CI2` with the base mangled as a `<name>`;
  argument forwarding; param-dropping base variants with a VTT for
  virtual-base inheritance; calls still pass arguments (clang); empty
  fields addressed by byte offset. IR identical to clang 19 on the corpus.
- Parser: an unqualified mem-initializer naming a template type parameter
  resolves at definition time ([temp.local]/6), so `H() : T(0)` works in
  instantiations.
- AST dump: `ConstructorUsingShadowDecl` / `UnresolvedUsingValueDecl`
  as clang (normalizer table too); constructor/destructor names of a class
  template pattern print as the injected-class-name (`Base<T>`), of a
  specialization bare; hidden shadows omitted.
- Tests: `conformance/class.inhctor.init/` (run-basic, run-forwarding,
  constexpr with an `oven:` budget, constexpr-fail, overload-access),
  `over.match.best/inherited-ctor.cpp`, `ast/class.inhctor.init/`,
  codegen `class_inheriting_ctors`, mangle `inheriting_ctors`.
- `<optional>` 25 → 8 errors; `<tuple>` 23 → 19 (with WP-141 merged; first
  blocker `sizeof...(T) >= 1` in a template argument).

## Deferred
- Deleted inheriting constructors for subobjects other than
  non-default-constructible ones (clang `ShouldDeleteSpecialMember`).
- Variadic base constructors (clang inlines the inheriting constructor).
- Oven construction of virtual bases (WP-077 follow-up).
- Repeated `using B::B;` in one class; `has_init_list_ctor` through
  shadows; [over.match.funcs]/4; the "non-literal type" constexpr note.
- Dump: a typedef used as a using qualifier is not marked `referenced`;
  injected-class-names are still not declared (implicit-mode goldens).
