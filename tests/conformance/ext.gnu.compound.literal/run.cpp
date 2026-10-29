// Scalar compound literals evaluate their initializer, in constant
// evaluation and at run time.
//@ expect: run
//@ stdout: 4 0 7 1 42 9
extern "C" int printf(const char*, ...);

constexpr int twice(int v) { return (int){v} * 2; }
int main() {
  static_assert(twice(21) == 42);
  static_assert((int){} == 0);
  int a = (int){4};
  long z = (long){};
  int s = (int){1} + (const int){2} * 3;
  bool b = (bool){a == 4};
  constexpr int k = (int){9};
  printf("%d %ld %d %d %d %d\n", a, z, s, b, twice(a + 17), k);
}
