// GNU `__extension__` silences extension diagnostics (the ones -pedantic
// enables) for the whole declaration it prefixes: a simple-declaration, a
// member-declaration, a function definition, a block declaration, and,
// as a unary operator, its operand; nested and repeated. Clean under
// -std=c++98 -pedantic-errors (GCC 16.1, clang 22). Found running GCC's
// C++ testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++98
//@ args: -pedantic-errors
//@ strict-diags
__extension__ char a[0];
__extension__ long long x;
__extension__ struct { int x; } sx;
__extension__ struct S {
  __extension__ long long m;
  __extension__ int z[0];
  __extension__ union { int p; float q; };
};
__extension__ int f() { long long q = 0; return (int)q; }
int g() {
  __extension__ long long y = 0;
  __extension__ char b[0];
  int v = __extension__ ({ 1; });
  return (int)(__extension__ y) + __extension__ (__extension__ 1LL) + v + (int)sizeof(b);
}
__extension__ __extension__ unsigned long long h();
