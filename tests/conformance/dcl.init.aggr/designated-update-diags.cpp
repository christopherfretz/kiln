// [dcl.init.aggr]/4, C99 6.7.9p19: a designator descending into a subobject
// initialized by an expression keeps that object and re-initializes only
// part of it: "initializer partially overrides prior initialization of this
// subobject" (-Winitializer-overrides) at the next designator, with the old
// initializer noted. Keeping the old object of a type with non-trivial
// destruction is an error. A full override whose prior initializer may have
// side effects notes that they will not occur.
//@ expect: compile-fail
//@ args: -Wno-c99-designator
//@ strict-diags
//@ diag: 37:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 37:14 note "previous initialization is here"
//@ diag: 38:37 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 38:23 note "previous initialization is here"
//@ diag: 39:25 warning "initializer overrides prior initialization of this subobject"
//@ diag: 39:14 note "previous initialization with side effects is here (side effects will not occur at run time)"
//@ diag: 40:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 40:14 note "previous initialization is here"
//@ diag: 40:32 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 40:14 note "previous initialization is here"
//@ diag: 44:22 error "initializer would partially override prior initialization of object of type 'T' with non-trivial destruction"
//@ diag: 44:14 note "previous initialization is here"
//@ diag: 48:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 48:14 note "previous initialization is here"
//@ diag: 50:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 50:14 note "previous initialization is here"
//@ diag: 54:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 54:14 note "previous initialization is here"
//@ diag: 56:25 warning "initializer overrides prior initialization of this subobject"
//@ diag: 56:14 note "previous initialization with side effects is here (side effects will not occur at run time)"
//@ requires: clang<22
// clang 22 no longer emits warning: initializer partially overrides prior initialization of this su...; WP-355 residual
struct A { int x, y; };
struct B { A a; int z; };
struct C { A arr[2]; };
A mk();
B b1 = {.a = mk(), .a.y = 3, .z = 4};
C c1 = {.arr = {mk(), mk()}, .arr[1].x = 5};
B b2 = {.a = mk(), .a = {.x = 1}};
B b3 = {.a = mk(), .a.x = 1, .a.y = 2};
struct T { int q; ~T(); };
struct H { T t; int w; };
T mt();
H h1 = {.t = mt(), .t.q = 1};
union U { int i; float f; };
struct D { U u; int k; };
U mu();
D d1 = {.u = mu(), .u.f = 1.0f};
struct S { char s[4]; };
S s1 = {.s = "ab", .s[1] = 'z'};
struct E : A { int w; };
struct F { E e; };
E me();
F f1 = {.e = me(), .e.w = 1};
int i1 = 1;
B b4 = {.z = i1++, .z = 2};
