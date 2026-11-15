// GCC's `-frange-for-ext-temps` in C++11 (spec/driver.md): the
// for-range-initializer's temporaries persist until the loop ends, a
// discarded left operand of a comma included (materialized as in C++17,
// [class.temporary]/7), while `__cpp_range_based_for` keeps C++11's
// 200907. Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++11
//@ args: -frange-for-ext-temps
//@ skip(clang): GCC's flag; clang has no way to turn P2718R0 on before C++23
//@ strict-diags
//@ stdout: S T1 1 2 ~L ~T1 ~S | 200907
extern "C" int printf(const char*, ...);
struct S {
  S() { printf("S "); }
  ~S() { printf("~S "); }
};
struct T {
  int id;
  T(int i) : id(i) { printf("T%d ", id); }
  ~T() { printf("~T%d ", id); }
};
struct L {
  int a[2];
  L() : a{1, 2} {}
  ~L() { printf("~L "); }
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
struct W {
  L l;
  const L& items() const { return l; }
};
W wrap(const T&) { return W(); }
int main() {
  for (int x : (S(), wrap(T(1)).items()))
    printf("%d ", x);
  printf("| %ld\n", long(__cpp_range_based_for));
}
