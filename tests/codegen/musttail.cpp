// clang `musttail` (WP-809): a marked return lowers to `musttail call`
// followed by `ret` on every path: scalar, void, small and sret
// aggregates, byval-passed aggregates, parenthesized calls, function
// pointers, non-virtual/virtual/static members, pointers to members and
// template recursion.
int f(int);
void v(int);
struct P { int a, b; };
P mp(int);
struct B { long a[4]; };
B mb(int);
int g(int x) { [[clang::musttail]] return f(x); }
void gv(int x) { [[clang::musttail]] return v(x); }
P gp(int x) { [[clang::musttail]] return mp(x); }
B gb(int x) { [[clang::musttail]] return mb(x); }
int g2(int x) { if (x) { [[clang::musttail]] return f(x); } return 0; }
int (*fp)(int);
int g3(int x) { [[clang::musttail]] return fp(x); }
int g4(int x) { [[clang::musttail]] return (f(x)); }
struct S {
  int m(int);
  static int sm(int);
  virtual int vm(int);
  int n(int x) { [[clang::musttail]] return m(x); }
  int vn(int x) { [[clang::musttail]] return vm(x); }
  int pn(int x);
};
int st(int x) { [[clang::musttail]] return S::sm(x); }
template <class T> T tt(T x) { if (x) { [[clang::musttail]] return tt<T>(x - 1); } return x; }
int ut(int x) { return tt(x); }
struct L { long a, b, c; };
L ml(L);
L gl(L x) { [[clang::musttail]] return ml(x); }
int use(S *s) { return s->n(1) + s->vn(2) + s->pn(3); }
int (S::*pm)(int) = &S::m;
int S::pn(int x) { [[clang::musttail]] return (this->*pm)(x); }
