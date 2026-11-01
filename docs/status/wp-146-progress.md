# WP-146 progress — inheriting-constructor follow-ups (WP-114)

Branch `wp/146-inheriting-ctor-followups`, worktree `kiln/.worktrees/wp-146`,
base af52e15 (main merged early for WP-149's `using.rs` signature). Every
behaviour was compared against clang 19.

Result: see "Gate and CI" below.

## Items
1. [x] **Deleted inheriting constructors.** `class/special/deletion.rs:205`
   `inheriting_ctor_should_delete` ports clang's `ShouldDeleteSpecialMember`
   with an `InheritedConstructorInfo`. The inherited bases are checked
   through their inherited constructors; every other subobject is checked
   for a deleted or inaccessible default constructor or destructor; reference
   and const members must be initialized. A use is diagnosed with
   `sema_deleted_inherited_ctor_use` (`class/special/define.rs:997`) and the
   "constructor inherited by" notes. `ctor_select.rs` reports the
   constructor as deleted, so `__is_constructible` is false.
   Test: `class.inhctor.init/deleted.cpp`.
2. [x] **Init-list constructors and [over.match.funcs]/4, /8 through
   shadows.** `class/ctor_select.rs:163` `ctors_and_inherited` covers both
   `has_init_list_ctor` and the [over.match.list] phase 1. Three small changes
   in `overload/*`:
   - `ics.rs:531` `object_ics_found`: /4, the class of the using-declaration
     is the type of the object parameter (clang `ActingContext`);
   - `ics.rs` slice check: /8 overrides arity and conversion failures, as
     clang does;
   - `diag.rs`: the slice note is kept for template candidates.

   Tests: `class.inhctor.init/init-list.cpp`, `class.inhctor.init/slice.cpp`,
   `over.match.funcs/using-object.cpp`.
3. [x] **Variadic base constructors, decided: clang's model.** The difference
   was observable:
   - kiln dropped the variadic arguments, which was a miscompile;
   - mangling and ABI differed.

   Changes:
   - codegen inlines a C-variadic inheriting constructor at each call, with
     no CI symbol: `codegen/class.rs:296` `inlines_inheriting_ctor` and
     `:311` `emit_inlined_inheriting_ctor`;
   - an inherited constructor template specialization is mangled with the
     base constructor's template arguments and the pattern's signature
     (`CI1 B I...E <pattern params>`): `abi/mangle.rs:721`
     `bare_function_type`, plus the new `Decls::inherited_ctor` map
     (`ast/decl.rs:2101`);
   - the base variant of a specialization is now `C2`, not `C1`:
     `mangle.rs:977` `template_prefix` takes the structor variant.

   Template-variadic constructors are inherited per specialization, as in
   clang, so `sizeof...` agrees.
   Tests: `codegen/class_inheriting_ctors_variadic.cpp` (IR byte-identical on
   both triples) and `mangle/inheriting_ctors.cpp`.
4. [x] **A repeated `using B::B;`.** Only `sema/using.rs` changed:
   - `class_using_declarator` (:200) checks `previous_inheriting_using` (:442,
     new) first. It emits "redeclaration of using declaration" and "previous
     using declaration", and creates no shadows, so nothing cascades.

   Test: `class.inhctor.init/repeated.cpp`.
5. [x] **Oven: virtual bases through inheriting constructors.** A class with
   a virtual base is never a literal type, so this cannot be a constant
   expression. The Oven traps first with clang's `CheckLiteralType` note:
   `oven/compile/class.rs` `construct` calls `:515` `has_virtual_bases`.
   Test: `class.inhctor.init/constexpr-vbase.cpp`.
6. [x] **"Non-literal type" for constexpr variables.** `sema/decl.rs:3086`
   `constexpr_var_virtual_base` emits clang's `RequireLiteralType` error and
   notes, one per virtual base in vbases() order, for a class or an array of
   one. Same test as item 5.
7. [~] **AST dump parity.**
   - A typedef used as a using-qualifier is already `referenced`, as in
     clang. Nothing to do.
   - Shadow-hiding timing already matches clang. Both declare the implicit
     constructors of a class with inherited constructors at `}`. I probed a
     never-constructed class, user constructors before and after, constructor
     templates, a class-template pattern, and implicit and explicit
     instantiations.
   - Injected-class-names: not done. They are synthesized in lookup, not
     declared. Declaring them touches lookup, member iteration and
     instantiation, so this is not cheap. `ast/class.implicit/eager.cpp`
     stays XFAIL.

## Follow-ups
- Inlining a C-variadic inheriting constructor as the base variant of a
  class with virtual bases reports "not supported yet". The VTT has to be
  threaded through.
- The non-literal check covers virtual bases only. Other non-literal classes
  keep the "non-constexpr constructor" note, and `constinit` keeps the
  inherited-constructor note.
- The constexpr-variable error comes after the initializer's errors. clang
  reports it at the declaration.
- An array aggregate in a constant evaluation names the element type (`'M'`).
  clang names the array (`'M[2]'`).
- In an instantiated class, clang lists the shadows replayed from a
  non-dependent pattern using-declaration in reverse. This affects dump and
  candidate order only.
- Injected-class-names in the AST (see item 7).
- `__builtin_va_start` and `va_start` are unsupported, so the variadic runtime
  test does not read its varargs.
- Seen in passing:
  - clang's `-Wuninitialized` "does not declare any constructor" warning;
  - "attempt to use a deleted function" for a deleted base or member
    destructor in constructors;
  - `static_assert` printing `'!c'`.

## Gate and CI
(filled in at handback)
