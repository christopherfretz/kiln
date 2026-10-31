// [class.temporary] before C++23: only the temporary bound directly to
// `__range` is extended; the other temporaries of the
// for-range-initializer are destroyed at the end of its full-expression,
// before the first iteration.
//@ expect: run
//@ std: c++20
//@ stdout: T1 ~T1 1 2 ~L |
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
  for (int x : make(T(1))) printf("%d ", x);
  printf("|\n");
}
