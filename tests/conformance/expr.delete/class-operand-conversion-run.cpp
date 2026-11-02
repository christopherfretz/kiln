// [expr.new]/8, [expr.delete]/2: a class-type array bound is converted
// to std::size_t by its conversion function and a class-type delete
// operand to a pointer by its conversion function, at run time and in
// constant evaluation.
//@ expect: run
//@ std: c++26
//@ stdout: 3 5 7
//@ strict-diags
#include <cstdio>

struct Count {
  int n;
  constexpr operator int() const { return n; }
};
struct Owner {
  int* p;
  constexpr operator int*() const { return p; }
};
struct TplCount {
  template <class T> constexpr operator T() const { return T(2); }
};

constexpr int sum() {
  int* a = new int[Count{3}];
  for (int i = 0; i < 3; ++i) a[i] = i + 1;
  int s = a[0] + a[1] + a[2];
  delete[] Owner{a};
  int* one = new int(4);
  s += *one;
  delete Owner{one};
  int* b = new int[TplCount{}];
  b[0] = b[1] = 0;
  delete[] Owner{b};
  return s;
}
static_assert(sum() == 10);

int main() {
  Count c{3};
  int* a = new int[c];
  for (int i = 0; i < c; ++i) a[i] = 2 * i + 3;
  Owner o{a};
  std::printf("%d %d %d\n", a[0], a[1], a[2]);
  delete[] o;
  int* b = new int[TplCount{}];
  delete[] Owner{b};
}
