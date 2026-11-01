Memory-limit fixtures (`spec/harness.md` "Resource limits"), run by
`xtask/src/test/tests.rs` with `xtask/fixtures/fake-hog/kiln` as the compiler
and no oracle. A `// fake-hog-mib: N` line makes the fake compiler allocate
N MiB. The fake identifies as kiln, so every compile's cap includes kiln's
1024 MiB stack reservation; test binaries get the bare cap. Each file's
expected status is its name prefix under the default `--mem-limit`.
