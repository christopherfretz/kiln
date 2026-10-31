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
- [ ] Parser: range-for detection, for-range-declaration, init-statement.
- [ ] Sema: array ranges, member begin/end, ADL begin/end + diagnostics, braced-list ranges, dependent ranges.
- [ ] P2718 extension of range-initializer temporaries.
- [ ] Oven, codegen.
- [ ] Tests, spec.

## Next step
Parser + array desugaring end to end.
