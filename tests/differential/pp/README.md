# Preprocessor differential corpus

Every test here is `expect: pp` with the clang oracle: `kiln -E` must produce
the same token stream as `clang -E` (rules: `spec/harness.md`
"pp comparison"). Parity is tracked on `docs/status/pp-parity.md`.

| Directory | Contents | Source |
|---|---|---|
| `libcxx/`, `libcxx/experimental/` | one test per libc++ 19 public header | generated |
| `libcxx-c/` | one test per C header libc++ wraps (`<stdio.h>`, ...) | generated |
| `sysroot/` | one test per top-level `/usr/include/*.h` that libc++ does not wrap | generated |
| `torture/` | hand-written tests of the preprocessor's hard corners, with goldens | hand-written |

## Generated tests

Do not edit them; regenerate:

```
cargo xtask gen-pp-corpus            # c++26 (the default)
cargo xtask gen-pp-corpus --std c++20  # adds <header>.c++20.cpp siblings
cargo xtask gen-pp-corpus --check    # exit 1 if the checked-in files are stale
```

The generator prints what it excludes and why (internal `__*` headers,
`ext/`, non-header files, and sysroot headers that cannot be included on
their own). The sysroot tests depend on the host's glibc and Linux headers
(Debian/Ubuntu layout); a header that is missing on a host fails the oracle
and shows up as an ERROR, not a kiln FAIL.

## Torture tests

File names start with the standard section they exercise (`cpp.*` for
[cpp], `lex.*` for [lex]). Auxiliary files live in `torture/Inputs/`. Each
test has a `.expected` golden of clang 19's output; after adding a test,
create an empty golden, run

```
cargo xtask test --compiler clang++-19 tests/differential/pp/torture --update
```

and review the golden. A test must pass clang against clang; mark a kiln
gap with `//@ xfail(kiln): <reason>`, never by weakening the test.
