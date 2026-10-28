`oven:` self-test fixtures, run with `--compiler xtask/fixtures/fake-kiln/kiln
--no-oracle` (see `xtask/src/test/tests.rs`). The fake kiln compiles with
clang and "reports" the canned `<test>.oven.jsonl` next to each test.
Outcome = file-name prefix, as in `../basic/`.
