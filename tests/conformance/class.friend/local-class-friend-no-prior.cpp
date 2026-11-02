// [class.friend]/11: an unqualified friend function declared in a local
// class must redeclare a function declared in the innermost enclosing
// non-class scope; scopes outside it are not considered. Clang suggests a
// namespace function with the same signature.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:17 error "no matching function found in local scope"
//@ diag: 24:17 error "no matching function 'outer' found in local scope; did you mean '::outer'?"
//@ diag: 18:6 note "'::outer' declared here"
//@ diag: 25:16 error "no matching function found in local scope"
//@ diag: 26:17 error "no matching function found in local scope"
//@ diag: 27:17 error "no matching function found in local scope"
//@ diag: 32:28 error "no matching function found in local scope"
//@ diag: 36:26 error "no matching function found in local scope"
//@ diag: 36:43 error "no matching function found in local scope"
//@ diag: 42:19 error "no matching function found in local scope"

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
