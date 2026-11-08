// [expr.ref]/7.3.2 [over.over]/1 [dcl.spec.auto.general]/4: a class member
// access naming only non-static member functions may only be called
// (clang's "reference to non-static member function must be called",
// with a "did you mean to call it with no arguments?" hint when every
// zero-argument candidate returns the same type and a "possible target
// for call" note per candidate); an overload set holding a static member
// does not deduce a placeholder unless it is a template-id naming a single
// static specialization. spec/sema-overload.md "Member overload sets",
// WP-788 (WP-823).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:15 error "reference to non-static member function must be called"
//@ diag: 43:24 note "possible target for call"
//@ diag: 49:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 32:7 note "possible target for call"
//@ diag: 33:7 note "possible target for call"
//@ diag: 50:11 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 32:7 note "possible target for call"
//@ diag: 33:7 note "possible target for call"
//@ diag: 51:11 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 38:24 note "possible target for call"
//@ diag: 52:14 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 38:24 note "possible target for call"
//@ diag: 53:5 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 32:7 note "possible target for call"
//@ diag: 33:7 note "possible target for call"
//@ diag: 55:8 error "variable 'a2' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 56:8 error "variable 'a3' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 57:8 error "variable 'a5' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
//@ diag: 58:8 error "variable 'a6' with type 'auto' has incompatible initializer of type '<overloaded function type>'"
struct G {
  int f();
  int f(int);
  static int s();
  static int s(int);
  int m();
  static int m(int);
  template <int N> int Ns() { return N; }
  template <class T> static int S2();
  template <class T> static int S2(int);
  template <int N> int Mx();
  template <int N> static int Mx(int);
  template <int N> int Ns1(int);
};
void t() {
  G g;
  // A set of only non-static members names a bound member function.
  auto a7 = g.Ns1<1>;
  int b1 = g.f;
  (void)g.f;
  (void)g.Ns<1>;
  int b4 = g.Ns<1>;
  g.f;
  // A set holding a static member, with no single static winner.
  auto a2 = g.s;
  auto a3 = g.m;
  auto a5 = g.S2<int>;
  auto a6 = g.Mx<1>;
}
