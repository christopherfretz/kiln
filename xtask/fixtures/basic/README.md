Runner self-test fixtures (not conformance tests). The expected outcome of
each file is the prefix of its name: `pass-`, `fail-`, `xfail-`, `xpass-`,
`skip-`, `error-`. `xtask/src/test/tests.rs` runs this directory with
`--compiler clang++-19 --timeout 1` and checks every outcome.
