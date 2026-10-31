# WP-106 progress — function templates remainder

Branch `wp/106-function-templates-remainder`. Implementer notes; the
supervisor folds the outcome into BACKLOG/HANDOFF.

## Deliverables
1. Explicit specialization / instantiation of function templates — IN PROGRESS
   - parser: `template<>` / `[extern] template` + non-class declaration now parsed
     (`at_spec_tag_or_template`); template-id declarator ids after explicit inst.
   - sema: `templ/funcs.rs::act_on_function_spec_declarator` — [temp.deduct.decl]
     matching via `deduce_funcaddr` + `more_specialized`; §4.5 transitions; explicit
     spec = unregistered redeclaration of the spec (`function_spec_redecl`).
   - `deduce_funcaddr` substitutes explicit args into the whole function type
     (clang's `could not match 'int (int *)' against 'int (int)'`), mismatch resugared.
   - members of class template specs (`template<> int A<int>::f()`,
     `template int A<char>::f();`, `extern template ...`): `member_of_spec_candidate`,
     `act_on_explicit_member_{spec,inst}` (MemberSpecInfo kind/poi transitions,
     clang's out-of-line no-match / after-instantiation errors).
   - `template<> template<>` heads, in-class explicit spec, operator template-ids
     (`template<> bool operator==<char>(...)`).
   - explicit head no longer leaks into members instantiated under it
     (`templ.explicit_depth` vs instantiation-stack depth).
   - GAP: `template<> template<class U> int A<long>::g(U)` (member template of an
     implicit spec redeclared as a template) still unimplemented.
   - GAP: declared-but-undefined explicit member spec can still be instantiated from
     the pattern; proposed one-line hook in class.rs (wp-109):
     `instantiate_function_definition` returns early when member_spec kind is
     ExplicitSpecialization.
2. TemplateParamObject — not started
3. Template candidates (ics/arrow/oper/address) — not started
4. std::initializer_list construction — not started
5. Q3 measurement, tests, spec — not started

## Interface changes
- `kiln_ast`: `AstBuilder::function_spec_redecl`, `DeclArena::respecify_function`,
  `AstBuilder::redecl_link` now `pub(crate)`.
- `kiln_diag`: `sema_fn_template_spec_no_match`, `sema_fn_template_spec_ambiguous`,
  `note_fn_template_spec_matched` (appended at the end of the table).
- `Sema::has_explicit_inst_head`, `act_function_def_next`, `operator_function_template`.
- `kiln_diag`: `sema_member_decl_no_match` (appended).
- codegen one-line hook: `module.rs::is_implicit_instantiation` returns false for a
  member whose MemberSpecInfo kind is ExplicitSpecialization (else linkonce_odr).

## Notes for WP-113
- Explicit instantiation definitions are emitted with External linkage; clang uses
  `weak_odr` + comdat (`_ZeqIcEb1CT_`, `A<char>::f`).
- `extern template int A<short>::g();` should suppress emission (kiln emits linkonce_odr).
- explicit inst definition of a member template spec of an implicit class spec
  (`template int A<short>::g<int>(int);`) is not emitted.

## Pre-existing bugs found (not WP-106)
- Out-of-line member definitions of class templates (`template<class T> int A<T>::g(){}`)
  are emitted as `@_ZN1A1gEv` (template args missing from the mangled name) on main
  too — duplicate symbols across TUs. Codegen/mangling owner.
