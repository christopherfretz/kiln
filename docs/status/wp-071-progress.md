# WP-071 progress (function templates)

Branch `wp/071-function-templates`. Updated at each WIP commit. The
design notes are spec/sema-templates.md "Landed (WP-071)" (before §11.3)
and spec/sema-overload.md §10.7.

## Works
- `deduce_call` / `deduce_explicit_only` (`templ/deduce_call.rs`):
  - explicit args, including too-many, which is a substitution failure;
  - [temp.deduct.call] adjustments;
  - defaults under SFINAE;
  - `Incomplete`;
  - sugared P in mismatch notes.
- Memoized specializations (`get_function_spec`) with lazy body
  instantiation.
- Template candidates in:
  - named calls, where `f<args>` is widened to the whole overload set;
  - member calls: implicit and explicit object, `e.f<args>`, static and
    instance;
  - non-reversed operator candidates (non-member and member).
- Partial ordering /2.5 (`templ/order.rs`, `OverloadCandidateSet::tmpl_order`).
- SFINAE: explicit-arg substitution failures, array of void/function/
  reference with a literal bound, pointer to reference, reference to
  void, `T::x` on a non-class, and detection idioms. Errors in bodies
  stay hard errors.
- `&f<int>` / uncalled `f<int>` naming the one specialization
  (`resolve_single_template_id`).
- Diagnostics: `candidate function [with T = int]`, CWG1391
  `candidate function template not viable`, `f<int>` names, deduction
  notes.
- Tests: 72 in `tests/conformance/templates/{deduce,order,sfinae}/`.
  Generator: the conversation scratchpad `gen/` (not committed); every
  compile-fail header is clang-19 output with strict-diags. The only
  xfail is `call-init-list`, because `std::initializer_list` objects
  cannot be built.

## Not yet (owners)
- Packs: WP-072. Constraints: WP-073. CTAD: WP-071b.
- Reversed and rewritten operator template candidates.
- Conversion function templates (`ics.rs`) and `operator->` templates
  (`arrow.rs`).
- Done: `&f` against a target type ([temp.deduct.funcaddr]) — deduction + more-specialized pick in `overload/address.rs::address_template_candidates`. The no-match diagnostic still reports "not yet implemented" (owner: WP-071 follow-up).
- Explicit specialization and explicit instantiation of function
  templates; `TemplateParamObject`.
- Q3 non-dependent sharing measurement.

## Next step
- Done: `&f` against a target type ([temp.deduct.funcaddr]) — deduction + more-specialized pick in `overload/address.rs::address_template_candidates`. The no-match diagnostic still reports "not yet implemented" (owner: WP-071 follow-up).
`overload/address.rs::resolve_address_of_overloaded`: deduce P = the
template's function type from A = the target function type.
