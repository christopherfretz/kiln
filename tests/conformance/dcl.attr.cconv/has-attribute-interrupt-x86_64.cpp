// [cpp.cond] `__has_attribute(interrupt)` is 1 on x86-64, as clang 22
// (clang 19 answers 0 for the target-specific spelling). spec/sema.md
// §4.5, WP-1745.
//@ args: --target=x86_64-unknown-linux-gnu
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
static_assert(__has_attribute(interrupt) && __has_attribute(__interrupt__));
static_assert(__has_cpp_attribute(gnu::interrupt));
