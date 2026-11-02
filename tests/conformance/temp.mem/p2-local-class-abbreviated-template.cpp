// [temp.mem]/2, [dcl.fct]/22: a function with a placeholder parameter
// type in a local class is an abbreviated member function template, so
// it is "templates cannot be declared inside of a local class", at the
// start of its declaration (attributes included) as there is no
// `template` keyword; an abbreviated friend is "templates can only be
// declared in namespace or class scope". The member template is invalid:
// a call finds no candidate ("no matching member function", no notes),
// and its body is still checked. clang 19 reports the first error
// without a location.
//@ expect: compile-fail
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
//@ diag: 27:5 error "templates cannot be declared inside of a local class"
//@ diag: 28:5 error "templates cannot be declared inside of a local class"
//@ diag: 29:5 error "templates cannot be declared inside of a local class"
//@ diag: 30:5 error "templates cannot be declared inside of a local class"
//@ diag: 31:5 error "templates can only be declared in namespace or class scope"
//@ diag: 34:7 error "templates cannot be declared inside of a local class"
//@ diag: 32:17 error "no matching member function for call to 'f'"
//@ diag: 38:5 error "no matching member function for call to 'f'"
//@ diag: 39:3 error "no matching function for call to 'h'"
//@ diag: 40:10 error "no matching member function for call to 'p'"
//@ diag: 44:5 error "templates cannot be declared inside of a local class"
void g() {
  struct L {
    void f(auto);
    static int h(auto x) { return x; }
    void k(int, auto...) {}
    [[nodiscard]] inline int n(const auto &x) { return x; }
    friend void fr(auto);
    void ok() { f(2); }
    struct N {
      void p(auto *) {}
    };
  };
  L l;
  l.f(1);
  L::h(1);
  L::N().p(&l);
}
void g2() {
  struct M {
    void f(auto x) { undeclared(x); }
  };
}
