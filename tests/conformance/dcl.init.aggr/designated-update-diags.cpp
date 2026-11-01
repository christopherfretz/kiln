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
//@ diag: 35:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 35:14 note "previous initialization is here"
//@ diag: 36:37 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 36:23 note "previous initialization is here"
//@ diag: 37:25 warning "initializer overrides prior initialization of this subobject"
//@ diag: 37:14 note "previous initialization with side effects is here (side effects will not occur at run time)"
//@ diag: 38:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 38:14 note "previous initialization is here"
//@ diag: 38:32 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 38:14 note "previous initialization is here"
//@ diag: 42:22 error "initializer would partially override prior initialization of object of type 'T' with non-trivial destruction"
//@ diag: 42:14 note "previous initialization is here"
//@ diag: 46:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 46:14 note "previous initialization is here"
//@ diag: 48:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 48:14 note "previous initialization is here"
//@ diag: 52:22 warning "initializer partially overrides prior initialization of this subobject"
//@ diag: 52:14 note "previous initialization is here"
//@ diag: 54:25 warning "initializer overrides prior initialization of this subobject"
//@ diag: 54:14 note "previous initialization with side effects is here (side effects will not occur at run time)"
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
