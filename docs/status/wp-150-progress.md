# WP-150 progress — WP-138 follow-ups

Branch `wp/150-138-followups`, worktree `kiln/.worktrees/wp-150`, base
b8af85f; main merged at a1d35d1 (bd51965) and 0dc6691 (7c3e507). The simd/ext_vector item went to WP-151. Also carries the
WP-155 item `<variant>:1141` (partial substitution of a non-type pack).

## Items
1. Pack partial substitution (§7.2 of `spec/sema-templates.md`).
   - [x] (a) Non-type pack kept while a type pack in the same pattern is
     dependent: new `SubstNonTypeTemplateParmPackExpr` (built in keep mode
     by `nttp_ref`, expanded by `subst_nttp_pack`). `<variant>:1141` is
     gone. Test: `templates/packs/temp.variadic-partial-subst-nttp-pack.cpp`.
   - [x] (b) Function parameter packs partially substituted stay one pack
     parameter (`param_infos` uses `element_ty`); deduction matches a kept
     `SubstTemplateTypeParmPack` element-wise (`DeducedArgs::pack_elem`).
     Length mismatches use clang's "from outer parameter packs" text.
     Bindings print packs as `U = <long>`. Tests:
     `temp.variadic-partial-subst-function-param-pack.cpp`,
     `temp.variadic-length-mismatch-outer.cpp`.
   - [x] (c) Requires-expression parameter packs: parameters get the
     `PACK` flag (`act_on_requires_param`); `requires_expr` expands them
     and keeps a requires-expression dependent when a requirement still
     names a retained level. Test:
     `templates/concepts/expr.prim.req-parameter-pack.cpp`.
2. [x] Out-of-class member partial specializations
   (`template<class T> template<class U> struct Outer<T>::Inner<U*>`).
   - 97275fc: WP-155's tag-qualifier parser hunk taken byte-identical
     (`parse_tag_qualifier`, the declarator scope in `parse_tag_specifier`).
   - dbf1a51, partial-spec branch only: `Substituter::member_template` →
     new `out_of_line_member_partials` (templ/member.rs); the template-id
     tag body parsed in the qualifier's scope (`parse_class_template_id_tag`,
     kiln_parse/src/decl.rs); `ClassBuild::outer` (class/mod.rs) restored by
     `act_on_finish_record_definition` (decl.rs), so an in-class-declared
     partial defined out of line no longer leaves the class current.
   - Tests: `temp.spec.partial.member/{out-of-line-member-partial-spec,
     member-partial-spec-declared-in-class,
     member-partial-spec-declared-after-instantiation,
     member-partial-spec-of-partial-spec}.cpp`.
3. [x] [over.match.ref] explicit conversion functions in direct-init
   reference binding (48f1c31).
4. [x] Undeclared builtins: GCC integer builtins (ffs, parity, clrsb,
   bitreverse, rotate), bcmp, wmemcpy/wmemmove, memset, mempcpy (cf9b173).

## Known gaps
- No [temp.deduct.call]/4 "deduced A" check: an element mismatch against
  a kept pack says "could not match" instead of clang's "deduced type …
  does not match adjusted type …".
- clang's "(at least N vs. M)" variant for explicitly specified packs.
- "no matching function" for a qualified callee (`S<int>::f(…)`) is
  reported at the name, clang at the start of the nested-name-specifier.
- Pre-existing, shared with in-class and namespace-scope partials:
  redefinition names the partial qualified (`'Outer::Inner<U *>'`, clang
  `'Inner<U *>'`) and its note points at an earlier declaration (clang: the
  earlier definition); a non-deducible member partial (`Inner<int>`) says
  "does not use any of its template parameters" (clang skips that check for
  a dependent template name and warns -Wunusable-partial-specialization).
- `template<class U> struct Outer<int>::Inner<U*>` (no `template<>`) is
  accepted; clang requires the empty list (WP-155 has the diagnostic for
  member classes).
