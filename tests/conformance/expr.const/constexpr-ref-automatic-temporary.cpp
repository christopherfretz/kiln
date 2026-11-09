//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
// [expr.const.init]/2 (C++26, P2686R5): a temporary whose lifetime a
// constexpr local reference extends ([class.temporary]/6) is
// constexpr-referenceable from that function, so the reference (or a
// constexpr structured binding, or a pointer through it) is a constant;
// a const temporary is usable in constant expressions ([expr.const]/4),
// also in a constexpr function's evaluation and in a template.
struct S { int a, b; };
constexpr int sum(const S &s) { return s.a + s.b; }
void f() {
  constexpr const int &r = 42;
  static_assert(r == 42);
  constexpr const S &s = S{1, 2};
  static_assert(s.b == 2 && sum(s) == 3);
  constexpr const auto &[x, y] = S{3, 4};
  static_assert(x + y == 7);
  constexpr const int *p = &r;
  static_assert(*p == 42 && p == &r);
  constexpr const int (&arr)[2] = {5, 6};
  static_assert(arr[1] == 6 && arr + 1 == &arr[1]);
  {
    static_assert(s.a == 1);  // a nested block is the same scope
  }
}
constexpr int h(int n) {
  constexpr const int &r = 42;
  constexpr const int *p = &r;
  return n ? h(n - 1) + *p : r;
}
static_assert(h(3) == 168);
template <class T> int t() {
  constexpr const T &r = T(7);
  static_assert(r == 7);
  return r;
}
int u() { return t<int>() + t<long>(); }
