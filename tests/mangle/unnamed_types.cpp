// <unnamed-type-name> ::= Ut [ <nonnegative number> ] _ (Itanium C++ ABI
// §5.1.5.6; [class.pre]/1, [dcl.typedef]/9): the unnamed classes and
// enumerations of one class or function without a typedef name for
// linkage are numbered in declaration order, anonymous unions included,
// across nested blocks; a typedef-named one and closure types are not
// counted. The n-th is `Ut_` for n = 1, else `Ut<n-2>_` (clang
// `mangleUnqualifiedName`, `ItaniumNumberingContext`). Instantiated
// members follow their pattern's order.
struct A {
  struct { int x; void f() {} } a;
  enum { E1 } e;
  union { int u; };
  struct { int y; void f() {} } b;
  typedef struct T_ { int z; void f() {} } T;
  struct { int w; void f() {} } c;
  auto l() { return [] {}; }
};
inline int g() {
  struct { int p; int f() { return p; } } s{1};
  { struct { int q; int f() { return q; } } t{2}; s.p += t.f(); }
  struct { int r; int f() { return r; } } u{3};
  return s.f() + u.f();
}
template <class T> struct B {
  struct { T x; void f() {} } a;
  struct { T y; void f() {} } b;
};
void use() {
  A a; a.a.f(); a.b.f(); a.c.f(); A::T t; t.f(); a.l()();
  g();
  B<int> b; b.a.f(); b.b.f();
}
