// [stmt.ranged]/1, [class.temporary]/7 (P2718R0, C++23): every temporary
// of the for-range-initializer persists until the end of the loop, not
// only the one bound to `__range`: a temporary function argument, one
// created inside a lambda called in it, one in an arm of a conditional;
// they are destroyed after the loop in reverse order of construction
// however the loop is left: at its end, by `break`, by `return`, and by
// an exception thrown from the body. `__cpp_range_based_for` is 202211L.
// Before C++23 see p2718-exits-cxx20-run.cpp.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ stdout: T1 1 2 ~L ~T1 | T2 1 ~L ~T2 | T3 1 ~L ~T3 r | T4 1 ~L ~T4 caught | T5 1 2 ~L ~T5 | T6 1 2 ~L ~T6 | 202211
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
const L& id(const L& l) { return l; }
struct W {
  L l;
  const L& items() const& { return l; }
};
W wrap(const T&) { return W{}; }

int ret() {
  for (int x : id(make(T(3)))) {
    printf("%d ", x);
    return x;
  }
  return 0;
}
void thrower() {
  for (int x : wrap(T(4)).items()) {
    printf("%d ", x);
    throw x;
  }
}
int main(int argc, char**) {
  for (int x : id(make(T(1))))
    printf("%d ", x);
  printf("| ");
  for (int x : wrap(T(2)).items()) {
    printf("%d ", x);
    break;
  }
  printf("| ");
  ret();
  printf("r | ");
  try { thrower(); } catch (int) { printf("caught | "); }
  for (int x : [](const T&) { return W{}; }(T(5)).items())
    printf("%d ", x);
  printf("| ");
  for (int x : argc > 0 ? id(make(T(6))) : id(make(T(7))))
    printf("%d ", x);
  printf("| %ld\n", long(__cpp_range_based_for));
}
