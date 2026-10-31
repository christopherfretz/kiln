# WP-107 progress: `__int128` end to end

- Most of `__int128` already existed (builtin ids 18/19, ranks, layout,
  mangling `n`/`o`, call classification, `i128` codegen, Oven
  arithmetic, `__SIZEOF_INT128__` from the captured predefines).
- The gap was the implicit typedefs `__int128_t` / `__uint128_t`
  (clang's `Sema::Initialize`): now declared at TU start on 64-bit
  targets (`kiln_sema/src/builtins.rs::declare_int128_typedefs`).
- `<type_traits>` probe: 10 -> 6 errors.

Next: conformance tests `tests/conformance/basic.fundamental/int128*.cpp`,
spec notes, full CI.
