# WP-089 progress (range-based for, [stmt.ranged])

Updated: 2026-10-31

## Baseline (main e990962, debug kiln, `-std=c++26 -stdlib=libc++ -fsyntax-only -ferror-limit=0`)
| header | errors | range-for errors |
|---|---:|---:|
| `<array>` | 96 | 1 |
| `<span>` | 69 | 1 |
| `<string_view>` | 160 | 1 |

## State
- [x] AST node `CXXForRangeStmt` (`RangeForData` side table), builder, dump, clang-ast normalizer.
- [x] Parser: range-for detection, for-range-declaration, C++20 init-statement; structured bindings reported as unsupported.
- [x] Sema (`range_for.rs`): array ranges, member begin/end, ADL begin/end with clang's diagnostics and notes, braced-list ranges, dependent ranges (pattern kept, rebuilt at instantiation).
- [x] P2718 extension of range-initializer temporaries (C++23+), reverse destruction order in codegen and the Oven; EH cleanups at materialization.
- [x] Oven constexpr loops; codegen exact against clang (`tests/codegen/range_for_{array,class,p2718}.cpp`).
- [x] 21 tests in `tests/conformance/stmt.ranged/` (19 pass, 2 xfail(kiln)); 3 AST goldens.
- [x] Pre-existing fixes found on the way: reference-relatedness through array element cv; deduced `auto` arrays keep `AutoType` sugar (`int &[2]`); dependent `auto` prints as `auto`.
- [x] Spec: sema.md §7.6, sema-class.md §5.4, ast.md, ast-dump.md.
- [x] `cargo xtask ci --quick` green (kiln corpus 1490 passed, 15 xfail, 1505 tests).
- [x] Header probe after (same flags): `<array>` 95 errors (0 range-for), `<span>` 68 (0), `<string_view>` 159 (0).
- [ ] Full `cargo xtask ci` (exit gate).

## Next step
Full `cargo xtask ci`, then hand back.
