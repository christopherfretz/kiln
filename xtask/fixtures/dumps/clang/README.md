`expect: layout` and `expect: vtables` fixtures with clang as the compiler
under test (and as the oracle, so no comparison is made): the dump must be
produced, the `diag:` lines must match, and the dump must not be empty.
Outcome = file-name prefix, as in `../../basic/`. Run with `--compiler
clang++-19`; `../kiln/` covers the comparison with the oracle.
