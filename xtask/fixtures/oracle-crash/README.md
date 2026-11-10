Oracle-retry fixtures (spec/harness.md "Oracle"): run with clang++-19 as the
compiler under test and the fake clang (`../fake-clang/clang++`) as the
oracle. The clang oracle's compile is retried once when it dies of a signal;
`pass-*` pass with an `advisory: oracle crashed once (signal 11), retry ok`
note, `error-*` stay ERROR (a second crash, or a failure that is never
retried). Outcome = file-name prefix, as in `../basic/`.
