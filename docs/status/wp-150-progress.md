# WP-150 progress — WP-138 follow-ups

Branch `wp/150-138-followups`, worktree `kiln/.worktrees/wp-150`, base
b8af85f. The simd/ext_vector item went to WP-151. Also carries the
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
2. [ ] Out-of-class member partial specializations.
3. [ ] [over.match.ref] explicit conversion functions.
4. [ ] Undeclared builtins (`__builtin_labs` and friends).

## Known gaps
- No [temp.deduct.call]/4 "deduced A" check: an element mismatch against
  a kept pack says "could not match" instead of clang's "deduced type …
  does not match adjusted type …".
- clang's "(at least N vs. M)" variant for explicitly specified packs.
- "no matching function" for a qualified callee (`S<int>::f(…)`) is
  reported at the name, clang at the start of the nested-name-specifier.
