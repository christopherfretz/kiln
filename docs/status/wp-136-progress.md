# WP-136 progress

Branch `wp/136-friend-template-ids`.

| Item | State | Where |
|---|---|---|
| 1. friend class template specializations (`friend struct X<T>;`, partial-spec friend error) | done | templ/spec.rs `act_on_class_template_id_tag`; class/access.rs `is_friend`; tests temp.friend/friend-class-specialization*.cpp |
| 2. deduced return type used before its definition is instantiated | already fixed by WP-142; test added | tests templates/auto/dcl.spec.auto.ret-on-demand.cpp |
| 3. default argument depending on a template parameter; braced default arguments | done | subst/decls.rs `default_args`; class/mod.rs `act_on_param_default_list`; kiln_parse class.rs `replay_method`; codegen fold.rs walks default arguments; test dcl.fct.default/dependent-and-braced.cpp |
| 4. concept-id / requires-expression at run time | done | kiln_codegen expr.rs (constant `i1`); test templates/concepts/concept-id-runtime-value.cpp |
| 5. `>=` in a template argument | verified; test added | temp.names/greater-equal-sizeof-dependent.cpp |

Known gap: an access check in a class template pattern that relies on a
befriended specialization is accepted at definition time and not
re-checked at instantiation (spec/sema-templates.md §4.8).
