//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects (and clang 22 lacks expansion statements)
// [stmt.expand]/5.2-5.3, [expr.const.init]/2 (C++26, P2686R5): with a
// constexpr for-range-declaration the range is a constexpr reference, so a
// glvalue range that is a local of the same function is bound by reference
// (not copied): elements designate the local's own subobjects, and a
// non-copyable range works. Also in a constexpr function's evaluation and a
// function template.
struct NC {
  int v[3];
  constexpr NC(int a, int b, int c) : v{a, b, c} {}
  NC(const NC &) = delete;
  constexpr const int *begin() const { return v; }
  constexpr const int *end() const { return v + 3; }
};
struct P { int a, b; };
void f() {
  constexpr NC nc(1, 2, 3);
  int i = 0;
  template for (constexpr const int &x : nc) {
    static_assert(&x == &nc.v[0] || &x == &nc.v[1] || &x == &nc.v[2]);
    ++i;
  }
  constexpr P p{4, 5};
  template for (constexpr const int &x : p) static_assert(&x == &p.a || &x == &p.b);
  constexpr int arr[2] = {6, 7};
  template for (constexpr const int &x : arr) static_assert(&x == arr || &x == arr + 1);
  (void)i;
}
constexpr int cf() {
  constexpr P p{1, 2};
  constexpr int arr[3] = {3, 4, 5};
  int s = 0;
  template for (constexpr int x : p) s += x;
  template for (constexpr int x : arr) s += x;
  template for (constexpr const int &x : arr) s += int(&x - arr);
  return s;
}
static_assert(cf() == 3 + 12 + 3);
template <class T> int t() {
  constexpr T v{8, 9};
  int s = 0;
  template for (constexpr const int &x : v) {
    static_assert(&x == &v.a || &x == &v.b);
    s += x;
  }
  return s;
}
int u() { return t<P>(); }
