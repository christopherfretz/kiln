// C17 6.7.6.3p11, 6.9.1p6: in a function declarator, an identifier list
// may reuse the name of a file-scope variable (`struct S f(s1, n)` with
// `s1` a variable): C has no direct-initializer, so the parentheses are
// never an initializer (clang runs `isCXXFunctionDeclarator` for C++
// only). An implicit-int K&R definition (`x(a, b) { ... }`, C89 3.5.2)
// takes its parameters as `int`. gcc.c-torture: compile/structret.c,
// compile/parms.c.
//@ expect: run
//@ std: c89
//@ args: -Wno-deprecated-non-prototype
//@ stdout: 7 3 9
int printf(const char *, ...);
struct S { int a, b; } s1, s2;
int n;

struct S pick(s1, n, s2)
     struct S s1, s2;
     int n;
{
  return n ? s1 : s2;
}

x(a, b) { return a + b; }

int main(void) {
  struct S p = { 7, 1 }, q = { 2, 9 };
  printf("%d %d %d\n", pick(p, 1, q).a, x(1, 2), pick(p, 0, q).b);
  return 0;
}
