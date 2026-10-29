// [dcl.fct.default]: default arguments make a candidate viable with
// fewer arguments ([over.match.viable]/2.3); redeclarations add
// defaults (/4).
//@ expect: compile-ok
char f(int, int = 0);
long f(double);
static_assert(__is_same(decltype(f(1)), char));
static_assert(__is_same(decltype(f(1.0)), long));
static_assert(__is_same(decltype(f(1, 2)), char));
short g(int, int);
short g(int, int = 1);
static_assert(__is_same(decltype(g(1)), short));
struct S {
  char m(int = 1, ...);
  long m(double);
  S(int = 0, int = 0);
};
static_assert(__is_same(decltype(S().m()), char));
static_assert(__is_same(decltype(S().m(1, 2, 3)), char));
static_assert(__is_same(decltype(S(1).m(1.0)), long));
S s1;
S s2(1);
S s3(1, 2);
int x = 3;
int h(int = x);
int y = h();
