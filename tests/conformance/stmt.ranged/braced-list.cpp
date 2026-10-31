// [stmt.ranged]/1 with a braced-init-list range: `__range` is deduced as
// `std::initializer_list<E>` ([dcl.spec.auto]), and class elements are
// copied into the loop variable.
//@ expect: run
//@ stdout: 6 | b5 b6 |
#include <initializer_list>
extern "C" int printf(const char*, ...);

struct T {
  int id;
  T(int i) : id(i) {}
};

int main() {
  int s = 0;
  for (int x : {1, 2, 3}) s += x;
  printf("%d | ", s);
  for (const T& t : {T(5), T(6)}) printf("b%d ", t.id);
  printf("|\n");
}
