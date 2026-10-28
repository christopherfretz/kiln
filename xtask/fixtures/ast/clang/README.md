`expect: ast` fixtures with clang as the compiler under test: the harness
normalizes clang's JSON dump (`xtask clang-ast`) and compares it with the
golden. Outcome = file-name prefix, as in `../../basic/`. Run with
`--compiler clang++-19`; `../kiln/` covers the kiln path.
