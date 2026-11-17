// GNU C: a flexible array member as a union member, or as the only named
// member of a structure (after unnamed bit-fields), initialized in a
// static object: the object holds the elements, past its size when it
// must (C mode shares the layout and constant code with C++:
// tests/conformance/ext.gnu.flexible.array/run-union-and-alone-static-init.cpp).
// Found running GCC's testsuite.
//@ std: c17
//@ expect: run
//@ args: -Wno-gnu-flexible-array-initializer -Wno-flexible-array-extensions -Wno-gnu-empty-struct
//@ stdout: 1 4 | 4 1 | 4 8 | 7 11 | 7 11 | 4 4 0 0 4
#include <stdio.h>
union U1 { int a; int b[]; } u1 = {.b = {1, 2, 3, 4}};
union U2 { int a; char b[]; } u2 = {.a = 0x01020304};
union U3 { unsigned char a[]; int b[]; } u3 = {.b = {0x01020304, 0x05060708}};
struct S1 { int b[]; } s1 = {{7, 11}};
struct S2 { unsigned : 2; unsigned : 3; int b[]; } s2 = {{7, 11}};
int main(void) {
  printf("%d %d | %d %d | %d %d | %d %d | %d %d | ", u1.b[0], u1.b[3], u2.b[0], u2.b[3],
         u3.a[0], u3.a[4], s1.b[0], s1.b[1], s2.b[0], s2.b[1]);
  printf("%d %d %d %d %d\n", (int)sizeof u1, (int)sizeof u2, (int)sizeof u3, (int)sizeof s1,
         (int)sizeof s2);
  return 0;
}
