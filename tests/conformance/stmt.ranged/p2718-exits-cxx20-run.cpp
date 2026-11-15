// [stmt.ranged]/1, [class.temporary]/6 before C++23 (P2718R0 not in
// effect): only the temporary bound to `__range` persists until the end
// of the loop; the for-range-initializer's other temporaries (a
// temporary argument) are destroyed at the end of its full-expression,
// before the first iteration, however the loop is then left.
// `__cpp_range_based_for` is 201603L. C++23 in p2718-exits-run.cpp.
//@ expect: run
//@ std: c++20
//@ strict-diags
//@ stdout: T1 ~T1 1 2 ~L | T2 ~T2 1 ~L | T3 ~T3 1 ~L r | T4 ~T4 1 ~L caught | T6 ~T6 1 2 ~L | 201603
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

int ret() {
  for (int x : make(T(3))) {
    printf("%d ", x);
    return x;
  }
  return 0;
}
void thrower() {
  for (int x : make(T(4))) {
    printf("%d ", x);
    throw x;
  }
}
int main(int argc, char**) {
  for (int x : make(T(1)))
    printf("%d ", x);
  printf("| ");
  for (int x : make(T(2))) {
    printf("%d ", x);
    break;
  }
  printf("| ");
  ret();
  printf("r | ");
  try { thrower(); } catch (int) { printf("caught | "); }
  for (int x : argc > 0 ? make(T(6)) : make(T(7)))
    printf("%d ", x);
  printf("| %ld\n", long(__cpp_range_based_for));
}
