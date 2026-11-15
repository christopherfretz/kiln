// [expr.prim.id.general]/2, [over.call.func]/3.1 (P2900R14): in a
// precondition of a constructor or a postcondition of a destructor of X,
// a potentially-evaluated member name made into `(*this).m`, and an
// unqualified call selecting a non-static member function, are
// ill-formed; `this->m`, static members and other contract assertions
// are fine. Found running GCC's C++ testsuite (g++.dg/contracts).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 22:11 error "a precondition of a constructor cannot access a member through an implicit 'this'; write 'this->' explicitly"
//@ diag: 22:31 error "a precondition of a constructor cannot access a member through an implicit 'this'; write 'this->' explicitly"
//@ diag: 22:64 error "a precondition of a constructor cannot access a member through an implicit 'this'; write 'this->' explicitly"
//@ diag: 22:72 error "a precondition of a constructor cannot access a member through an implicit 'this'; write 'this->' explicitly"
//@ diag: 23:13 error "a postcondition of a destructor cannot access a member through an implicit 'this'; write 'this->' explicitly"
//@ diag: 29:11 error "a precondition of a constructor cannot access a member through an implicit 'this'; write 'this->' explicitly"
struct B { bool bb; bool bf() const; };
struct C : B {
  bool b;
  bool a() const;
  static bool s();
  C() pre(b) pre(this->b) pre(a()) pre(this->a()) pre(s()) pre(bb) pre(bf()) {}
  ~C() post(a()) post(this->a()) pre(a()) {}
  void m() pre(b) post(a()) {}
};
template <class T> struct D {
  T b;
  bool a() const;
  D() pre(b) pre(this->a()) {}
};
D<bool> d;
