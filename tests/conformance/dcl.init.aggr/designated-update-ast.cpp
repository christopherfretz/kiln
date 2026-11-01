//@ expect: ast
//@ oracle: clang
//@ args: -Wno-initializer-overrides -Wno-c99-designator
// [dcl.init.aggr]/4, C99 6.7.9p19: a designator naming part of a subobject
// whose prior initializer is not a braced list keeps that initializer and
// re-initializes only the designated part. The subobject's initializer is
// a DesignatedInitUpdateExpr: the prior initializer, then an updater list
// whose untouched slots are NoInitExpr (an array's past its last updated
// element are its NoInitExpr filler; a union's names one member).
struct A { int x, y; };
struct B { A a; int z; };
struct C { int arr[4]; int t; };
union U { int i; float f; };
struct D { U u; int k; };
struct E : A { int w; };
struct F { E e; };
A mk();
U mu();
B b1 = {.a = mk(), .a.y = 3};
C c1 = {.arr = {1, 2, 3, 4}, .arr[1] = 9};
D d1 = {.u = mu(), .u.f = 1.0f, .k = 2};
F f1 = {.e = E{}, .e.w = 4};
B b2[2] = {[0] = {.a = mk()}, [0].a.x = 11, [1].z = 4};
B b3 = {.a = A{1, 2}, .a.x = 5, .a.y = 6};
struct S { char s[8]; int n; };
S s1 = {.s = "hello", .s[1] = 'a', .s[3] = 'p'};
// Within an updater, a further designated subobject is an updater too; a
// braced list there, and a list merged into, keep their own fillers.
struct H { A a; A b; };
struct G { C c; };
struct I { H h; };
C mkc();
H mkh();
G g1 = {.c = mkc(), .c.arr[1] = 9};
G g2 = {.c = mkc(), .c.arr = {7}};
G g3 = {.c = mkc(), .c.arr = {7}, .c.arr[2] = 8};
G g4 = {.c = mkc(), .c.arr[1] = 9, .c.arr[3] = 1};
I i1 = {.h = mkh(), .h.b = {3}};
I i2 = {.h = mkh(), .h.b.y = 3, .h.a.x = 1, .h.b.x = 4};
