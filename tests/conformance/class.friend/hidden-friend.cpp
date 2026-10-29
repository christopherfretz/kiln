// [class.friend]/7, [basic.lookup.argdep]/4: a friend first declared in a
// class is not found by ordinary lookup until it is redeclared at namespace
// scope; a qualified friend must name an existing function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:6 error "no member named 'f' in namespace 'N'"
//@ diag: 20:12 error "no member named 'g' in namespace 'N'"
//@ diag: 24:18 error "friend declaration of 'nope' does not match any declaration in namespace 'N'"
namespace N {
struct A {
  friend void f();
  friend void g(A) {}
  friend void h();
};
void h();
}
void use(N::A a) {
  N::f();
  g(a);
  (void)N::g(a);
  N::h();
}
struct B {
  friend void N::nope();
};
