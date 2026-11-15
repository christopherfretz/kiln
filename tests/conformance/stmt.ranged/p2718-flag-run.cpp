// GCC's `-frange-for-ext-temps` (spec/driver.md): P2718R0's extension of
// every for-range-initializer temporary ([class.temporary]/7) before
// C++23, and `__cpp_range_based_for` 202211L with it. The opposite flag
// is p2718-no-flag-run.cpp. Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++17
//@ args: -frange-for-ext-temps
//@ skip(clang): GCC's flag; clang has no way to turn P2718R0 on before C++23
//@ strict-diags
//@ stdout: T1 1 2 ~L ~T1 | 202211
extern "C" int printf(const char*, ...);
struct T {
  int id;
  T(int i) : id(i) { printf("T%d ", id); }
  ~T() { printf("~T%d ", id); }
};
struct L {
  int a[2] = {1, 2};
  ~L() { printf("~L "); }
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
struct W {
  L l;
  const L& items() const& { return l; }
};
W wrap(const T&) { return W{}; }
int main() {
  for (int x : wrap(T(1)).items())
    printf("%d ", x);
  printf("| %ld\n", long(__cpp_range_based_for));
}
