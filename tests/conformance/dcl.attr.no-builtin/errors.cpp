// [dcl.attr.grammar]/5 The clang `no_builtin` attribute's checks as
// clang 19 (`handleNoBuiltinAttr`, `ActOnFunctionDeclarator`): only a
// function definition takes one (not a declaration, nor a defaulted or
// deleted function); each argument is a string literal naming a library
// builtin (`std-<name>` a standard one), an invalid name warns and is
// dropped; the wildcard does not compose with names, across attributes
// too. spec/sema.md §5.4, WP-808.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:16 error "no_builtin attribute is permitted on definitions only"
//@ diag: 26:27 warning "'foo' is not a valid builtin name for 'no_builtin'"
//@ diag: 27:27 warning "'*' is not a valid builtin name for 'no_builtin'"
//@ diag: 28:27 warning "'*' is not a valid builtin name for 'no_builtin'"
//@ diag: 29:56 error "empty 'no_builtin' cannot be composed with named ones"
//@ diag: 30:27 error "expected string literal as argument of 'no_builtin' attribute"
//@ diag: 32:18 error "no_builtin attribute has no effect on defaulted or deleted functions"
//@ diag: 33:18 error "no_builtin attribute has no effect on defaulted or deleted functions"
//@ diag: 35:18 error "no_builtin attribute is permitted on definitions only"
//@ diag: 39:16 warning "'no_builtin' attribute only applies to functions"
//@ diag: 42:21 warning "'std-memcpy' is not a valid builtin name for 'no_builtin'"
//@ diag: 42:35 warning "'move' is not a valid builtin name for 'no_builtin'"
//@ diag: 43:21 warning "'__builtin_memcpy' is not a valid builtin name for 'no_builtin'"
__attribute__((no_builtin)) void d1();
void d1() {}
__attribute__((no_builtin("memcpy"))) void d2() {}
__attribute__((no_builtin("foo"))) void d3() {}
__attribute__((no_builtin("*"))) void d4() {}
__attribute__((no_builtin("*", "memcpy"))) void d5() {}
__attribute__((no_builtin, no_builtin("memcpy"))) void d6() {}
__attribute__((no_builtin(1))) void d7() {}
struct S {
  __attribute__((no_builtin)) S() = default;
  __attribute__((no_builtin)) void f() = delete;
  __attribute__((no_builtin("memset"))) void g() {}
  __attribute__((no_builtin("memset"))) void h();
};
void S::h() {}
[[clang::no_builtin("memset")]] void d8() {}
__attribute__((no_builtin)) int v;
[[clang::no_builtin("memcpy", "memcpy")]] void d9() {}
[[clang::no_builtin("std-move")]] void d10() {}
[[clang::no_builtin("std-memcpy", "move")]] void d11() {}
[[clang::no_builtin("__builtin_memcpy")]] void d12() {}
auto l = []() __attribute__((no_builtin)) {};
template <class T> __attribute__((no_builtin("memset"))) void t(T) {}
void use() { t(1); }
