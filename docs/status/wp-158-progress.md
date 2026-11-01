# WP-158 progress: consteval/class follow-ups (from WP-137)

Branch `wp/158-consteval-class-followups`, worktree `kiln/.worktrees/wp-158`.

## Done
- **(a)** `[[no_unique_address]]` member of class type: stored as its
  class's base-subobject type (`kiln_abi/src/irtype.rs:368`, new helper
  `overlapping_class` at `:623`; the only irtype.rs functions touched are
  `lower_struct` and that helper). Instantiated fields keep their
  attributes (`templ/subst/decls.rs`, `templ/subst/anon.rs`). Tests:
  `codegen/class_no_unique_address{,_init}.cpp`,
  `dcl.attr/no-unique-address-template.cpp`.
- **(b, c)** No elision into a base or delegating target (CWG2403, clang
  `RequireActualConstructor`): `class/construct.rs:157`,
  `class/ctor_select.rs:60` (`resolve_ctor_ex` skips CWG2327 conversion
  candidates), aggregate base elements use `NonVirtualBase` (C2)
  (`init.rs:618`); a base from a call copies the data size
  (`kiln_codegen/src/expr.rs:2026`). Tests:
  `codegen/class_base_init_call.cpp`,
  `class.base.init/consteval-base-copy.cpp`.
- **(h)** Immediate calls in implicit member initializers escalate the
  implicit constructor (`consteval.rs:198` `note_member_init_full_expr`,
  `class/special/define.rs:690` `finish_implicit_init`). Test:
  `dcl.constexpr/consteval-implicit-init-escalation.cpp`.
- **(f)** Already fixed by WP-153; constant-expression cases added
  (`expr.const/builtins-srcloc-aggregate-const.cpp`).
- **(j)** `-Wunused-value`: constructor conversions are checked through
  the constructor call; nodiscard constructors/classes give clang's
  `ignoring temporary created by a constructor` (`expr.rs:566`
  `unused_warning`). Test: `stmt.expr/unused-value-constructor.cpp`.
- **(g)** Global `operator new`/`delete` declared on unqualified or
  `::`-qualified lookup of their names (`expr.rs:837`). Test:
  `basic.stc.dynamic/implicit-global-new-delete.cpp`.
- **(e)** `extern template`: no member definitions at -O0, D1 calls,
  external vtable and static data members; templates/instantiations have
  no key function (`kiln_codegen/src/module.rs:487`
  `is_explicit_inst_declaration`, used by `emit_global_fn`, `inline_def`,
  `class.rs` `structor_def`/`emit_global_structor`; `vtable.rs:153,169`).
  Test: `codegen/class_extern_template_vtable.cpp` (both triples).
- **(d)** Generic lambda conversion escalation per specialization
  (`lambda/generic.rs:84`); the specialization is named
  `operator auto (*)(int)` as clang; the reason note is given for a
  generic call-operator specialization (`consteval.rs:601`). Tests:
  `dcl.constexpr/consteval-generic-lambda-reason.cpp`,
  `expr.prim.lambda/consteval-conversion-generic.cpp` (xfail(clang):
  clang 19 accepts).

## Follow-ups (not done)
- Explicit instantiation *definitions* emit `linkonce_odr` members and
  vtable where clang uses `weak_odr`.
- Member class of a class template specialization prints as
  `struct.S::I` instead of `struct.S<int>::I` in IR type names.
- Direct calls to replaceable `::operator new` lack clang's call-site
  `noalias nonnull` return and `allocsize(0)` (the new-expression path
  has them).
- An aggregate base element initialized from a non-prvalue or by
  value-initialization still queues the complete-object variant (same
  IR, emission order may differ).
- Conversion function template specializations other than a lambda's
  keep the pattern's name (`operator type-parameter-0-0`).
