// [expr.prim.this]/2: `this` may appear in the declarator of a member
// function after its cv-qualifier-seq, but a friend function declared in a
// class is no member: `this` is not in scope in its trailing return type
// or noexcept-specifier.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 12:34 error "invalid use of 'this' outside of a non-static member function"
//@ diag: 13:38 error "invalid use of 'this' outside of a non-static member function"
struct S {
  int v;
  friend auto f(S s) -> decltype(this->v);
  friend void g(S s) noexcept(sizeof(this));
  auto h() -> decltype(this->v);
  void k() noexcept(sizeof(this) > 0);
};
