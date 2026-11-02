// [class.friend]/11: an unqualified friend function declared in a local
// class must redeclare a function declared in the innermost enclosing
// non-class scope; scopes outside it are not considered. Clang suggests a
// namespace function with the same signature.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:17 error "no matching function found in local scope"
//@ diag: 26:17 error "no matching function 'outer' found in local scope; did you mean '::outer'?"
//@ diag: 20:6 note "'::outer' declared here"
//@ diag: 27:16 error "no matching function found in local scope"
//@ diag: 28:17 error "no matching function found in local scope"
//@ diag: 29:17 error "no matching function found in local scope"
//@ diag: 34:28 error "no matching function found in local scope"
//@ diag: 38:26 error "no matching function found in local scope"
//@ diag: 38:43 error "no matching function found in local scope"
//@ diag: 44:19 error "no matching function found in local scope"
//@ requires: clang<22
// clang 22 emits error: cannot define friend function in a local class definition; WP-355 residual

void outer();
void f() {
  void g();
  struct L {
    friend void g();
    friend void h();
    friend void outer();
    friend int k(int);
    friend void g(int);
    friend void d() {}
  };
}
void l() {
  auto lam = [] {
    struct M { friend void q(); };
  };
}
template <class T> void t() {
  struct N { friend void r(); friend void s(T); };
}
template void t<int>();
struct Q {
  void m() {
    struct P {
      friend void z();
    };
  }
};
