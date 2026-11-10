GCC-as-the-compiler-under-test fixtures (spec/harness.md "GCC as the
compiler under test"): run with the fake GCC (`../fake-gcc/g++`) as
`--compiler`. Outcome = file-name prefix, as in `../basic/`. The fake GCC
rejects the kiln/clang flags GCC 16.1 rejects, so these also check that the
harness passes GCC's spelling.
`Inputs/` holds the other translation unit of the `unit:` test, which the
harness builds in two steps under GCC too (WP-1261); `skip-flags-*` is the
flag probe's skip of a test whose flags GCC rejects.
