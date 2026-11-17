// [temp.pre]/1: `inline` before a template-declaration is clang 22's
// -Wstatic-inline-explicit-instantiation warning, so -Werror makes it an
// error; any other decl-specifier there is "expected unqualified-id".
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ args: -Werror
//@ strict-diags
//@ diag: 10:1 error "ignoring 'inline' keyword on explicit template instantiation"
//@ diag: 11:11 error "expected unqualified-id"
inline template <class T> void f1();
constexpr template <class T> void f2();
