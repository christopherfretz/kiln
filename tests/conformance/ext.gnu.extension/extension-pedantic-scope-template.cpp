// GNU `__extension__` in a template: the silencing belongs to the
// declaration's tokens, and kiln checks a zero-length array of dependent
// type once, in the definition, so the instantiation reports nothing
// (GCC 16.1 too). clang 22 reports -Wzero-length-array again at each
// instantiation. Found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++98
//@ args: -pedantic-errors
//@ strict-diags
//@ xfail(clang): clang 22 re-issues the extension warning at instantiation
template<class T> struct S { int n; __extension__ T arr[0]; };
S<int> s;
template<class T> int tf() { __extension__ T arr[0]; return (int)sizeof(arr); }
int u = tf<int>();
