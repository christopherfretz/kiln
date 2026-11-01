//@ expect: ast
// [dcl.init.aggr]/3.1, [dcl.init.list]: nested and anonymous-member
// designators (C99 extensions in C++) descend into the subobject; the
// following non-designated items continue there; a re-designated
// subobject merges with its prior initialization; array designators
// leave an array filler for the holes.
struct Pt { int x; int y; int z; };
struct Q { Pt p; int w; };
struct V { int m; struct { int k; int n; }; int r; };
union U { int i; float f; };
struct A { int k; };
struct H { union { A a; int b; }; int w; };
struct W { U u; int a[3]; };
Q q1 = {.p.y = 1, .p.z = 2};
Q q2 = {.p.y = 1, 7, 8};
Q q3 = {.p = {.y = 1}, .p.x = 2};
V v1 = {.k = 1, 2, 3};
V v2 = {.n = 1, .r = 3};
H h1 = {.a = A{.k = 1}, .w = 2};
H h2 = {.b = 2};
W w1 = {.u.f = 1.0f, .a[1] = 2};
int ar[4] = {[2] = 1, 5, [0] = 3};
