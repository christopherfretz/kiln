GCC-as-the-compiler-under-test fixtures (spec/harness.md "GCC as the
compiler under test"): run with the fake GCC (`../fake-gcc/g++`) as
`--compiler`. Outcome = file-name prefix, as in `../basic/`. The fake GCC
rejects the kiln/clang flags GCC 16.1 rejects, so these also check that the
harness passes GCC's spelling.
