// [class.temporary]/6.4 (C++23, P2718R0): every temporary of the
// for-range-initializer lives until the end of the loop, so `T(1)` and
// the `L` returned through `id` are destroyed after the last iteration,
// in reverse order of construction.
//@ expect: run
//@ std: c++23
//@ stdout: T1 1 2 ~L ~T1 |
//@ stdout: T2 1 2 ~L ~T2 |
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

int main() {
  for (int x : make(T(1))) printf("%d ", x);
  printf("|\n");
  for (int x : id(make(T(2)))) printf("%d ", x);
  printf("|\n");
}
