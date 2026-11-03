GCC-oracle fixtures (spec/harness.md "GCC oracle"): run with the fake kiln
(`../fake-kiln/kiln`) as the compiler under test and the fake GCC
(`../fake-gcc/g++`) as `--gcc`. Outcome = file-name prefix, as in `../basic/`;
`pass-advisory-*` pass with an `advisory: oracle crashed (...)` note.
