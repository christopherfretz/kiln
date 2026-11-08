// [dcl.attr.grammar]/5 An invalid builtin name in `no_builtin` warns
// (clang `handleNoBuiltinAttr`); clang 22 names `[[clang::no_builtin]]`
// with its scope, `__attribute__((no_builtin))` without one.
// spec/sema.md §5.4, WP-808.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:21 warning "'move' is not a valid builtin name for 'clang::no_builtin'"
//@ diag: 12:27 warning "'foo' is not a valid builtin name for 'no_builtin'"
//@ diag: 13:3 warning "'clang::no_builtin' attribute only applies to functions"
[[clang::no_builtin("move")]] void f() {}
__attribute__((no_builtin("foo"))) void g() {}
[[clang::no_builtin]] int v;
