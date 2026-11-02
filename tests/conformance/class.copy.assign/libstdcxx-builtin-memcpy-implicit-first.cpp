// [class.copy.assign]/12.2, [dcl.link]/6, [over.match.best]/2: when the
// implicitly-defined copy assignment of a trivially copyable array member
// is the first use of `__builtin_memcpy`, the builtin it declares is the
// one a later written call finds, and a user redeclaration with the
// builtin's type redeclares it: one function, never an ambiguous call
// (libstdc++ cluster X, WP-332). Companion:
// libstdcxx-builtin-memcpy-written-first.cpp.
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 4 ab ab
extern "C" int printf(const char *, ...);
struct A { int a[4]; };
void g(A &x, const A &y) { x = y; }
void f(char *d, const char *s) { __builtin_memcpy(d, s, 2); }
extern "C" void *__builtin_memcpy(void *, const void *, decltype(sizeof 0));
void h(char *d, const char *s) { ::__builtin_memcpy(d, s, 2); }
struct C { char c[2]; };
void j(C &x, const C &y) { x = y; }
int main() {
  A x{}, y{{1, 2, 3, 4}};
  g(x, y);
  char p[3] = {}, q[3] = {}, s[3] = "ab";
  f(p, s);
  h(q, s);
  printf("%d %s %s\n", x.a[3], p, q);
}
