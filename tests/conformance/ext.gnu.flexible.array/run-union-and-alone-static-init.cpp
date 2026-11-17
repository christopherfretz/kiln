// GNU: a flexible array member as a union member, or as the only named
// member of a structure (after unnamed bit-fields), initialized in a
// static object: the object holds the elements, past its size when it
// must. kiln dropped the elements of a union's flexible array member (the
// union's constant took the member's zero-length type; a union of only
// flexible arrays, of size 0, was even initialized at run time past its
// end) and of a structure whose flexible array follows unnamed
// bit-fields (the list's initializers were matched to the members with
// the bit-fields counted). Sizes and values as clang 22 and GCC 16.1.
// Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: run
//@ args: -Wno-gnu-flexible-array-initializer -Wno-c99-designator -Wno-flexible-array-extensions
//@ stdout: 1 4 | 4 1 | 4 8 5 | 7 11 | 7 11 | 9 | 4 4 0 0 4 4 | 0 0 4 0
#include <cstddef>
#include <cstdio>
union U1 { int a; int b[]; } u1 = {.b = {1, 2, 3, 4}};
union U2 { int a; char b[]; } u2 = {.a = 0x01020304};
union U3 { unsigned char a[]; int b[]; } u3 = {.b = {0x01020304, 0x05060708}};
struct S1 { int b[]; } s1 = {{7, 11}};
struct S2 { unsigned : 2; unsigned : 3; int b[]; } s2 = {{7, 11}};
union U4 { short s; long long b[]; };
int main() {
  static U4 u4 = {.b = {9}};
  std::printf("%d %d | %d %d | %d %d %d | %d %d | %d %d | %lld | ", u1.b[0], u1.b[3],
              u2.b[0], u2.b[3], u3.a[0], u3.a[4], u3.b[1] >> 24, s1.b[0], s1.b[1],
              s2.b[0], s2.b[1], u4.b[0]);
  std::printf("%zu %zu %zu %zu %zu %zu | ", sizeof u1, sizeof u2, sizeof u3, sizeof s1,
              sizeof s2, alignof(U1));
  std::printf("%zu %zu %zu %zu\n", offsetof(U1, b), offsetof(U3, b), offsetof(S2, b),
              offsetof(S1, b));
}
