//@ expect: ast
//@ ast-filter: S::S
// [class.base.init]/7, /9, /13: the ctor-initializer list in
// initialization order: virtual bases, direct bases, then members, each
// from its mem-initializer (direct- or list-initialization; a base
// constructor call has its construction kind), its default member
// initializer (CXXDefaultInitExpr) or default-initialized; `()`
// value-initializes; an anonymous union member is initialized through its
// IndirectField (clang prints the field); delegation ([class.base.init]/6)
// is the only initializer. Matches clang 19.
struct B { B(int); B(); };
struct V { int v; };
struct C { C(int, int); ~C(); };
struct A { int a; int b; };
struct E { E(); };
struct S : B, virtual V {
  int x;
  int &r;
  C c;
  A agg;
  long y = 3;
  E e;
  union { int u; float f; };
  S(int i) : B(i), V{2}, x(i), r(x), c(1, 2), agg{1, 2}, u(4) {}
  S() : S(1) {}
  S(long l) : x{}, r(x), c{3, 4}, agg(5, 6), y(l), e(), f() {}
  S(char k) : B(), x(k), r(x), c(k, k) {}
};
