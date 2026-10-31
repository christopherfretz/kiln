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
   - TODO: members of class template specs (`template<> void A<int>::f()`),
     `template<> template<>`, in-class explicit spec, tests.
2. TemplateParamObject — not started
3. Template candidates (ics/arrow/oper/address) — not started
4. std::initializer_list construction — not started
5. Q3 measurement, tests, spec — not started

## Interface changes
- `kiln_ast`: `AstBuilder::function_spec_redecl`, `DeclArena::respecify_function`,
  `AstBuilder::redecl_link` now `pub(crate)`.
- `kiln_diag`: `sema_fn_template_spec_no_match`, `sema_fn_template_spec_ambiguous`,
  `note_fn_template_spec_matched` (appended at the end of the table).
- `Sema::has_explicit_inst_head`.

## Notes for WP-113
- Explicit instantiation definitions are emitted with External linkage; clang uses
  `weak_odr` + comdat.
