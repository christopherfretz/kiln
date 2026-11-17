// GNU: a constexpr union whose flexible array member is initialized holds
// its elements past the union's size, as a structure's do: the constant
// evaluator makes the member active and leaves the elements to the static
// initializer (a member access of an element is not a constant
// expression: array-to-pointer decay of an array member without known
// bound, as clang says), so elements of class type, more bytes than the
// union, and a union larger than the elements are all accepted, and the
// static data holds the elements with clang 22's and GCC 16.1's bytes.
// kiln rejected elements of class type ("not supported yet"). The
// rejected side is union-constexpr-elements-errors.cpp.
//@ std: c++20
//@ expect: run
//@ args: -Wno-gnu-flexible-array-initializer -Wno-c99-designator -Wno-flexible-array-extensions
//@ stdout: 3 6 3 | 4 4 8
#include <cstdio>
struct E {
  int x;
  constexpr E(int v) : x(v) {}
};
union U5 { int a; E b[]; };
constexpr U5 u5 = {.b = {E(1), E(2), E(3)}};
union U6 { char c; int b[]; };
constexpr U6 u6 = {.b = {1, 2, 3, 4, 5, 6}};
union U7 { long long a; int b[]; };
constexpr U7 u7 = {.b = {1, 2, 3}};
int main() {
  const E *e = u5.b;
  const int *i6 = u6.b, *i7 = u7.b;
  std::printf("%d %d %d | %zu %zu %zu\n", e[2].x, i6[5], i7[2], sizeof u5, sizeof u6,
              sizeof u7);
}
