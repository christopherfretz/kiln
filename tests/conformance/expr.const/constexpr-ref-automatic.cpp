//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
// [expr.const.init]/2 (C++26, P2686R5): an automatic object is
// constexpr-referenceable from a point in the same function parameter scope,
// so a constexpr reference or pointer in that function may designate it, and
// [expr.const]/8-9 make it usable in constant expressions there.
struct Q { int a, b; };
constexpr const int &id(const int &x) { return x; }
void f(int parm) {
  constexpr Q q{1, 2};
  constexpr const Q &r = q;
  static_assert(r.a == 1);
  constexpr const int *p = &q.a;
  static_assert(*p == 1 && p + 1 == &q.b);
  int i = 3;
  constexpr int &ri = i;               // non-const referent: the address is constant
  static_assert(&ri == &i);
  constexpr int *pp = &parm;           // a parameter is in the same scope
  static_assert(pp == &parm);
  constexpr const int &rid = id(q.b);  // through a constexpr function
  static_assert(rid == 2);
  constexpr int arr[2] = {1, 2};
  constexpr const int *pa = arr;
  constexpr const int &r1 = pa[1];
  static_assert(*pa == 1 && r1 == 2);
  {
    constexpr const Q &inner = q;      // a nested block is the same scope
    static_assert(inner.b == 2);
  }
  [] {                                 // a lambda's own locals, from its body
    constexpr Q lq{3, 4};
    constexpr const Q &lr = lq;
    static_assert(lr.b == 4);
  }();
}
template <class T> void t() {
  constexpr T v{5, 6};
  constexpr const T &rv = v;
  static_assert(rv.a + rv.b == 11);
}
template void t<Q>();
