# WP-149 progress — libc++ gate unblockers, batch 3

Branch `wp/149-gate-unblockers-3`, worktree `kiln/.worktrees/wp-149`, base
5ea147b. The first-error and any-error counts come from the WP-142 reading of
`docs/status/libcxx-m3.md`.

## Items
1. [x] **Operator and conversion function names in using-declarators**
   (28 first, 40 any; `using B::operator=;` through
   `_LIBCPP_VARIANT_DESTRUCTOR`). The parser (`decl.rs:380`
   `parse_using_declaration`) parses an operator-function-id or a
   conversion-function-id and an optional `...`. `Sema::act_on_using_declarator`
   now takes a `DeclName` and the ellipsis (`using.rs:90`, a signature
   change). Dependent members are re-declared on instantiation, including
   pack expansions over a dependent base (`templ/class.rs:1116`
   `member_using`).
   Tests: `namespace.udecl/operator-names.cpp`, `pack-expansion.cpp`.
2. [x] **Destructor template-ids** `p->~X<T>()` (8 first):
   `expr.rs:858` parses the template-id as a type and calls
   `Sema::rebuild_member_destructor`. Test: `class.dtor/template-id.cpp`.
3. [x] **Block-scope using-directives and using-declarations in templates**
   (108 any; `convert_to_timespec.h:30`): `templ/subst/stmts.rs:452`
   (the `UsingDirective` arm) and `:416` (`local_using_decl`).
   Test: `namespace.udir/block-scope-template.cpp`.
4. [x] **`_Atomic ( type-id )`** (83 any; `cxx_atomic_impl.h:274`):
   `decl.rs:567` and `:615` in the parser, and `deduce.rs:103`
   `act_on_atomic_type` (`sema_atomic_specifier_bad_type`). Substitution is
   in `templ/subst/types.rs`. Test: `ext.c11.atomic/atomic-specifier.cpp`.
   The bare `_Atomic` qualifier is still unsupported. The builtins and the
   semantics are not part of this item (see Follow-ups).
5. [x] **Explicit object parameters** (P0847; 58 any; `format_arg.h:292`).
   - Declarations are checked against [dcl.fct]/6 in
     `decl.rs:820` `explicit_object_member`, which sets FnSpec
     `explicit_object`. Instantiation keeps the flag through
     `ParamData::EXPLICIT_OBJECT`.
   - `this` in the body gives `sema_explicit_object_this`.
   - Calls (`class/member_expr.rs:769` `explicit_object_call`) pass the
     object as argument 0, and templates deduce `Self` from it
     (`overload/call.rs`).
   - `&C::f` is a plain function pointer (`member_pointer_of`;
     `is_implicit_object_member` for [over.over]).
   - Codegen calls these functions directly
     (`kiln_codegen/src/class.rs:1458`).
   - The tentative parser reads `this T` as a parameter
     (`tentative.rs:737`).
   - Gaps: lambdas (`sema_unsupported`), and the location of the qualifier
     diagnostic, which kiln reports at `)` where clang points at the
     qualifier.
   - Tests: `dcl.fct/explicit-object-call.cpp`, `explicit-object-diags.cpp`.
6. [ ] Braced default arguments: WP-136 did this. Verified after the merge
   (see below).
7. [ ] The WP-142 divergences. All three are filed, not fixed: none of them
   affects valid code, so no gate test depends on them.
   - (a) Not a divergence. clang 19 also accepts an explicit
     instantiation of a static data member whose declared type differs
     (`template float S<int>::x;` for `long x`).
   - (b) The extra `expected ';'` after an undeclared `w<int>` comes from
     the parser. It does not treat an undeclared name followed by `<` as a
     template-name in a declarator ([temp.names]/3), so it stops at `<`.
   - (c) A constexpr static member is loaded rather than folded. The
     result is the same.

## Follow-ups (not in scope)
- Atomic builtins: `__c11_atomic_*`, `__atomic_*`, `__atomic_always_lock_free`
  and `__c11_atomic_is_lock_free`. These are 95+ any errors once `_Atomic(T)`
  parses. WP-144's row names only the libm builtins.
- `_Atomic(T)` semantics: the Oven layout has no `TypeView::Atomic` arm
  (`sizeof` is 0), and Sema has no `NonAtomicToAtomic` or
  `AtomicToNonAtomic` conversions (so initializing `_Atomic(int)` from
  `int` is rejected).
- `vector:769`: in an NSDMI `__compressed_pair<pointer, allocator_type>(...)`,
  the `,` is read as a declarator separator.
- `__functional/function.h:377`: the composite pointer type of `void*` and
  `const T*`. `function.h:927`: the out-of-line constructor of a partial
  specialization, `function<_Rp(_ArgTypes...)>::function`.
- `variant:1141`: a member alias template with a pack expansion
  ("pack expansion in this context"). This is now the first `<variant>`
  error.
- `__is_trivially_constructible` and `__is_trivially_assignable` are not
  implemented.
