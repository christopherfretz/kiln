# WP-072 progress (packs)

## Works
- `templ/pack.rs`: probe-based expansion (`expand_pattern`, `expand_list`,
  `expand_types`, `expand_exprs`, `map_params`, `fold`, `size_of_pack`).
- Expansion in template-arg lists (`Substituter::args` / `expand_arg`), function
  type params, call args / paren lists (`expr_list`), braced lists,
  mem-initializer arg lists, type-trait args; function parameter packs in
  `param_infos` (`DeclOrPack::Pack`), pattern->spec param mapping.
- Folds (all 4 forms, empty-pack rules, iterative), `sizeof...` (lowered to an
  `IntegerLiteral` of `size_t` -- gap vs clang's dump).
- Length-mismatch diag `sema_pack_length_mismatch`.

## Next
- Deduction of packs in `deduce_call.rs` (trailing function param packs).
