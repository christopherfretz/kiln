// [expr.const]/10.8: the valid twin of volatile-member-access-clang22.cpp.
// A volatile member makes only its own accesses non-constant: the other
// members, a whole-object copy (the trivial copy constructor is not a read
// of the member through a volatile glvalue in clang) and construction and
// destruction stay constant. WP-1019.
//@ std: c++26
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
struct S { int a; volatile int v; };
constexpr int other() {
  S s{1, 2};
  s.a = 3;
  return s.a;
}
static_assert(other() == 3);
constexpr int heap() {
  auto *p = new S{4, 5};
  int r = p->a;
  delete p;
  return r;
}
static_assert(heap() == 4);
struct D {
  int x;
  volatile int v;
  constexpr D() : x(6), v(7) {}
  constexpr ~D() {}
};
constexpr int structors() {
  D d;
  auto *p = new volatile D;
  delete p;
  return d.x;
}
static_assert(structors() == 6);
