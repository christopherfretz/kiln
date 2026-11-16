// [dcl.init.general]/16.5, [expr.new]/7 (P0960, P1009): `new C[](8, 9)`
// allocates as many elements as there are arguments, so no element is
// value-initialized and `C` needs no default constructor (GCC 16.1
// accepts; clang 22 rejects with "no matching constructor").
//@ skip(clang): clang 22 value-initializes an element past the arguments of an omitted bound
//@ std: c++20
//@ expect: run
//@ stdout: 8 9
extern "C" int printf(const char *, ...);
struct C { int v; C(int x) : v(x) {} };
int main() {
  C *c = new C[](8, 9);
  printf("%d %d\n", c[0].v, c[1].v);
  delete[] c;
}
