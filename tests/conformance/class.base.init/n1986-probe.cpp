// [class.base.init]/6 (C++11, N1986): delegating constructors; the
// object is constructed once the target constructor completes. WP-646 probe.
//@ expect: run
//@ std: c++11
//@ strict-diags
//@ stdout: T1 B2 T1 B2 T3 B4 ~ ~ .
extern "C" int printf(const char*, ...);
struct S {
  int v;
  S(int x) : v(x) { printf("T%d ", x); }
  S() : S(1) { printf("B%d ", v + 1); }
  S(double) : S() { v = 3; printf("T%d ", v); }
  S(const char*) : S(3.0) { printf("B%d ", v + 1); }
  ~S() { printf("~ "); }
};
struct C { int a; constexpr C(int x) : a(x) {} constexpr C() : C(42) {} };
static_assert(C().a == 42, "constexpr delegation");
int main() {
  {
    S a;
    S b("x");
  }
  printf(".\n");
}
