// GCC's `-fno-range-for-ext-temps` (spec/driver.md): even in C++23 only
// the temporary bound to `__range` persists until the end of the loop
// (the rule before P2718R0), and `__cpp_range_based_for` is 201603L.
// The opposite flag is p2718-flag-run.cpp.
//@ expect: run
//@ std: c++23
//@ args: -fno-range-for-ext-temps
//@ skip(clang): GCC's flag; clang has no way to turn P2718R0 off in C++23
//@ strict-diags
//@ stdout: T1 ~T1 1 2 ~L | 201603
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
L make(const T&) { return L{}; }
int main() {
  for (int x : make(T(1)))
    printf("%d ", x);
  printf("| %ld\n", long(__cpp_range_based_for));
}
