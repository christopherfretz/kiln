# WP-072 progress (packs)

## Works
- `templ/pack.rs`: probe-based expansion (`expand_pattern[_at]`, `expand_list`,
  `expand_types`, `expand_exprs`, `map_params`, `fold`, `size_of_pack`,
  `element_ty`). Length mismatch reported at the ellipsis (clang parity).
- Expansion sites: template-arg lists, function type params, call args / paren
  lists, braced lists (`S{ts...}`), paren init (`P p(ts...)`), mem-initializer
  pack expansions (`B(ts)...`) and their arg lists, base-specifier packs,
  type-trait args, function parameter packs.
- Folds (all 4 forms, empty-pack rules, iterative); `stress/fold-65536.cpp`
  passes (Oven + codegen; run test, oracle none).
- `sizeof...` (lowered to an `IntegerLiteral` of `size_t` -- dump gap).
- Deduction: trailing function parameter packs, explicit pack prefixes,
  packs in template-arg lists (class/function/NTTP), conflict note in clang
  form (`<int> vs. <char>`), undeduced pack = empty.
- Variadic partial specializations (incl. of a variadic primary), ordering.
- `__make_integer_seq` / `__type_pack_element` with expanded packs.
- 51 tests in `tests/conformance/templates/packs/` + the stress test.

- Pack indexing (P2662): types and function parameter packs, clang's
  bounds errors (5 tests + run test).
- `<type_traits>`: 20 -> 18 errors (rest: WP-073 concepts, WP-107 int128).

## Not done
- Pack index that stays dependent with a known pack (member alias template).
- `using Ts::operator()...;` (parser: using-declarator kind unsupported).
- Partially substituted pack expansions (SubstTemplateTypeParmPack): stub.

## Next
- Full `cargo xtask ci` (running), then hand back. Spec note in §11.2 done.
