Runner self-test fixtures for `expect: debugger` (spec/harness.md
"Debugger tests"). The expected outcome of each file is the prefix of its
name, as in `../basic/`. `xtask/src/test/tests.rs`
(`debugger_fixtures_with_clang`) runs this directory with
`--compiler clang++-19` when `gdb` and `lldb-19` are installed.
