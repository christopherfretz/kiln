# WP-071 progress (function templates)

Branch `wp/071-function-templates`. Updated at each WIP commit.

## Works
- `deduce_call` (`kiln_sema/src/templ/deduce_call.rs`): explicit args
  (checked + substituted into P under an `ExplicitArgSubstitution` SFINAE
  frame), arity, per-(P,A) [temp.deduct.call] adjustments (reference P,
  forwarding ref + lvalue, decay + drop cv, cv intersection for reference
  P, IGNORE_QUALIFIERS for pointer A, DERIVED_CLASS), braced lists via
  `initializer_list<E>`, overload-set args (single non-template), defaults
  under SFINAE, `Incomplete`.
- `get_function_spec` (`templ/funcs.rs`): memoized spec decl
  (`builder().function_spec`), signature substituted
  (`Substituter::function_spec_signature`); failures memoized as INVALID
  with the first error in `templ.fn_spec_failures`.
- Overload: `add_template_candidate` (overload/call.rs); named calls with and
  without explicit args; `BadDeduction` notes (`deduction_note`): conflicting,
  incomplete, underqualified, non-deduced mismatch, substitution failure
  `[with T = int]: msg`, invalid explicit arg, instantiation depth.
- Lazy body instantiation (`instantiate_function_spec_definition`), codegen
  of specs works (mangling `_Z2idIiET_S0_`).

## Not yet
- member/operator/address-of template candidates; /2.5 more-specialized
  tie-break; explicit specialization/instantiation of function templates;
  TemplateParamObject; tests; xfail flips; spec notes.

## Next step
Run `cargo xtask ci --quick`, fix fallout, then member.rs / oper.rs.
