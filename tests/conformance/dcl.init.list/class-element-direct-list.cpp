// [dcl.init.list]/3.9: a direct list initializes a scalar from a class-type
// element by direct-initialization, so an explicit conversion function is a
// candidate; `bool{nullptr}` is direct-initialization too.
//@ expect: run
//@ stdout: 42 7 42 0
#include <cstdio>
struct X { explicit operator int() const { return 42; } };
struct S { operator short() const { return 7; } };
int main() {
  X x;
  int a{x};
  long b{S{}};
  auto c = int{x};
  bool d{nullptr}; // clang: -Wnull-conversion
  std::printf("%d %ld %d %d\n", a, b, c, d);
}
