// [dcl.attr.grammar]/6: clang 19 spells `no_sanitize` only
// `__attribute__((no_sanitize))` and `[[clang::no_sanitize]]`, so
// `[[gnu::no_sanitize]]` is an unknown attribute, ignored with a warning
// (WP-667; spec/sema.md §4.5). clang 22 knows it:
// gnu-no-sanitize-clang22.cpp.
//@ requires: clang<22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:3 warning "unknown attribute 'no_sanitize' ignored"

[[gnu::no_sanitize("undefined")]] int f1();
