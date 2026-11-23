// [expr.unary.op]/3, [over.over]/1: in a member function of a derived
// class, `&B::f` of an overloaded member function forms a pointer to
// member, the target selecting the overload; a static member template's
// specialization named through the implicit `this` (`s<0>`, `B::s<1>`) is
// a reference to the static function. Written extents (spec/ast.md §21):
// the qualified references begin at the qualifier `B` and the
// address-of at its `&`, a template-id ends at its `>`, as clang's
// `DeclRefExpr`. `cargo xtask extents` compares them with clang
// (WP-2462).
//@ expect: compile-ok

struct B {
  int f() { return 1; }
  int f(int x) { return x; }
  template <int N> static int s(int) { return N; }
  template <int N> int s(long) { return 10 * N; }
};

struct D : B {
  int h() {
    int (B::*p)() = &B::f;
    int (B::*q)(int) = &B::f;
    int (*r)(int) = &B::s<3>;
    return (this->*p)() + (this->*q)(2) + s<0>(0) + B::s<1>(0) + r(0);
  }
};

int use() { return D().h(); }
