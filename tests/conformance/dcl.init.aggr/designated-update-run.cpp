// [dcl.init.aggr]/4, C99 6.7.9p19: a designator naming part of a subobject
// whose prior initializer is not a braced list keeps that initializer, run
// once, and overwrites only the designated part; the other parts keep the
// prior initializer's values (locals at run time, globals by dynamic
// initialization).
//@ expect: run
//@ args: -Wno-initializer-overrides -Wno-c99-designator
//@ stdout: 1 3 0 2
//@ stdout: 1 9 3 4 5
//@ stdout: 6 1
//@ stdout: 11 2 0 0 4 3
//@ stdout: hallo 0
//@ stdout: 5 7 8 3
//@ stdout: 9 8 4
extern "C" int printf(const char*, ...);
struct A { int x, y; };
struct B { A a; int z; };
struct C { int arr[4]; int t; };
union U { int i; float f; };
struct D { U u; int k; };
struct S { char s[8]; int n; };
struct E : A { int w; };
struct F { E e; };
int calls;
A mk() { ++calls; return A{1, 2}; }
A mk2() { ++calls; return A{5, 6}; }
C mkc() { return C{{1, 2, 3, 4}, 5}; }
E mke() { return E{{9, 8}, 7}; }
B g1 = {.a = mk2(), .a.y = 7, .z = 8};
int main() {
  B b1 = {.a = mk(), .a.y = 3};
  printf("%d %d %d %d\n", b1.a.x, b1.a.y, b1.z, calls);
  struct G { C c; } g = {.c = mkc(), .c.arr[1] = 9};
  printf("%d %d %d %d %d\n", g.c.arr[0], g.c.arr[1], g.c.arr[2], g.c.arr[3], g.c.t);
  D d = {.u = U{.i = 4}, .u.i = 6, .k = 1};
  printf("%d %d\n", d.u.i, d.k);
  B b2[2] = {[0] = {.a = mk()}, [0].a.x = 11, [1].z = 4};
  printf("%d %d %d %d %d %d\n", b2[0].a.x, b2[0].a.y, b2[0].z, b2[1].a.x, b2[1].z, calls);
  S s = {.s = "hello", .s[1] = 'a'};
  printf("%s %d\n", s.s, s.n);
  printf("%d %d %d %d\n", g1.a.x, g1.a.y, g1.z, calls);
  F f = {.e = mke(), .e.w = 4};
  printf("%d %d %d\n", f.e.x, f.e.y, f.e.w);
}
