// [expr.ref], [expr.prim.id.general]: a bound member function, or an
// overload set of non-static member functions, used where a value is needed
// is "reference to non-static member function must be called"; clang
// recovers by calling it with no arguments when that call works (the hint
// "did you mean to call it with no arguments?"), so the follow-on
// diagnostics are about the call's result. Placeholder deduction reports
// the use twice (it diagnoses a copy of the initializer), a braced
// initializer once (WP-832).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 68:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 68:8 error "cannot initialize a variable of type 'int *' with an rvalue of type 'int'"
//@ diag: 69:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 69:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 70:12 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 71:11 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 72:13 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 73:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 73:8 error "variable has incomplete type 'void'"
//@ diag: 74:13 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 75:34 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 76:40 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 76:13 error "reinterpret_cast from 'S' to 'G::S *' is not allowed"
//@ diag: 77:13 error "reference to non-static member function must be called"
//@ diag: 81:13 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 82:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 83:5 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 84:12 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 84:12 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 85:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 86:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 58:7 note "possible target for call"
//@ diag: 59:7 note "possible target for call"
//@ diag: 86:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 58:7 note "possible target for call"
//@ diag: 59:7 note "possible target for call"
//@ diag: 87:13 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 58:7 note "possible target for call"
//@ diag: 59:7 note "possible target for call"
//@ diag: 88:20 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 89:9 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 90:13 error "reference to non-static member function must be called"
//@ diag: 60:7 note "possible target for call"
//@ diag: 61:7 note "possible target for call"
//@ diag: 91:15 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 92:5 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 93:17 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 94:12 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 58:7 note "possible target for call"
//@ diag: 59:7 note "possible target for call"
//@ diag: 94:8 error "cannot initialize a variable of type 'int *' with an rvalue of type 'int'"
//@ diag: 95:12 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
struct G {
  int f();
  int h(int);
  void v();
  int o();
  int o(int);
  int p(int);
  int p(long);
  struct S { int x; };
  S s();
};
void use(int);

void inits(G g) {
  int *p = g.f;
  auto a = g.f;
  auto b{g.f};
  int c{g.f};
  int k = g.f;
  auto w = g.v;
  int x = g.s.x;
  float f = static_cast<float>(g.f);
  G::S *q = reinterpret_cast<G::S *>(g.s);
  int m = g.h;
}

int operators(G g) {
  int a = g.f + 1;
  int b = -g.f;
  g.f;
  auto c(g.f);
  decltype(g.f) d = 0;
  auto e = g.o;
  int f = g.o;
  int h = true ? g.f : 0;
  use(g.f);
  int i = g.p;
  bool j = !g.f;
  g.v;
  int k = sizeof(g.f);
  int *r = (g.o);
  return g.f;
}
