# WP-107 progress: `__int128` end to end

Status: complete pending full `cargo xtask ci`.

- Most of `__int128` already existed on main (builtin ids 18/19, rank 7,
  layout 16/16, mangling `n`/`o`, call classification, `i128` codegen,
  Oven 128-bit arithmetic and overflow notes, `__SIZEOF_INT128__` from the
  captured clang predefines, so pp-parity is unchanged).
- The gap was the implicit typedefs `__int128_t` / `__uint128_t`
  (clang's `Sema::Initialize`): now declared at TU start on 64-bit
  targets (`kiln_sema/src/builtins.rs::declare_int128_typedefs`, called
  from `Sema::with_abi`).
- `<type_traits>` probe: 10 -> 6 errors (the 2 undeclared `__int128_t` /
  `__uint128_t` and their 2 "template argument ... must be a type"
  follow-ons are gone; what remains is WP-072 packs and alias
  instantiation).
- New tests: `tests/conformance/basic.fundamental/int128-{typedefs,
  constexpr,overflow,run}.cpp`, `tests/codegen/int128_ops.cpp` (IR-exact
  on both triples), `tests/mangle/int128.cpp`.
- Spec: `spec/types.md` §20, `spec/abi.md` §8.8.
- Found, not fixed (general, not `__int128`): Oven notes omit
  `(aka '...')` for typedefs; `-Wshift-count-overflow` /
  `-Wdivision-by-zero` fire in a constexpr variable initializer where
  clang is silent.
