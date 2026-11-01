// [dcl.init.aggr]/3.1, [dcl.init.list]/3.1: designators out of declaration
// order (-Wreorder-init-list, at the later initializer, note at the earlier
// one), mixed designated and non-designated items, nested and array
// designators (C99 extensions, -Wc99-designator, once per list), and an
// initializer overriding a prior one (-Winitializer-overrides). Nested and
// anonymous-member designators descend into the subobject and merge with
// its prior initialization.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 62:9 warning "nested designators are a C99 extension"
//@ diag: 63:9 warning "nested designators are a C99 extension"
//@ diag: 63:34 warning "ISO C++ requires field designators to be specified in declaration order; field 'w' will be initialized after field 'p'"
//@ diag: 63:24 note "previous initialization for field 'w' is here"
//@ diag: 64:22 warning "ISO C++ requires field designators to be specified in declaration order; field 'w' will be initialized after field 'p'"
//@ diag: 64:14 note "previous initialization for field 'w' is here"
//@ diag: 65:23 warning "initializer overrides prior initialization of this subobject"
//@ diag: 65:15 note "previous initialization is here"
//@ diag: 66:22 warning "nested designators are a C99 extension"
//@ diag: 67:9 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 67:17 note "first non-designated initializer is here"
//@ diag: 68:22 warning "ISO C++ requires field designators to be specified in declaration order; field 'V::(anonymous struct at "
//@ diag: 68:9 note "previous initialization for field 'V::(anonymous struct at "
//@ diag: 69:9 warning "nested designators are a C99 extension"
//@ diag: 69:24 warning "array designators are a C99 extension"
//@ diag: 70:9 warning "nested designators are a C99 extension"
//@ diag: 70:26 warning "initializer overrides prior initialization of this subobject"
//@ diag: 70:16 note "previous initialization is here"
//@ diag: 71:19 warning "nested designators are a C99 extension"
//@ diag: 71:21 warning "array designators are a C99 extension"
//@ diag: 72:24 warning "nested designators are a C99 extension"
//@ diag: 74:22 warning "nested designators are a C99 extension"
//@ diag: 74:29 warning "initializer overrides prior initialization of this subobject"
//@ diag: 74:18 note "previous initialization is here"
//@ diag: 75:24 warning "nested designators are a C99 extension"
//@ diag: 75:31 warning "initializer overrides prior initialization of this subobject"
//@ diag: 75:20 note "previous initialization is here"
//@ diag: 76:22 warning "initializer overrides prior initialization of this subobject"
//@ diag: 76:14 note "previous initialization is here"
//@ diag: 77:43 warning "ISO C++ requires field designators to be specified in declaration order; field 'b' will be initialized after field 'a'"
//@ diag: 77:35 note "previous initialization for field 'b' is here"
//@ diag: 77:51 warning "initializer overrides prior initialization of this subobject"
//@ diag: 77:35 note "previous initialization is here"
//@ diag: 78:23 warning "ISO C++ requires field designators to be specified in declaration order; field 'y' will be initialized after field 'x'"
//@ diag: 78:15 note "previous initialization for field 'y' is here"
//@ diag: 79:23 warning "ISO C++ requires field designators to be specified in declaration order; field 'z' will be initialized after field 'x'"
//@ diag: 79:15 note "previous initialization for field 'z' is here"
//@ diag: 80:13 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 80:10 note "first non-designated initializer is here"
//@ diag: 81:10 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 81:18 note "first non-designated initializer is here"
//@ diag: 81:26 warning "initializer overrides prior initialization of this subobject"
//@ diag: 81:18 note "previous initialization is here"
//@ diag: 82:14 warning "array designators are a C99 extension"
//@ diag: 82:14 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 82:23 note "first non-designated initializer is here"
//@ diag: 83:13 warning "array designators are a C99 extension"
struct Pt { int x; int y; int z; };
struct Q { Pt p; int w; };
struct V { int m; struct { int k; int n; }; int r; };
union U { int i; float f; };
struct W { U u; int a[3]; };
Q a1 = {.p.y = 1, .p.x = 2};
Q a2 = {.p.y = 1, .w = 2, .p.x = 3};
Q a3 = {.w = 1, .p = {1}};
Pt a4 = {.x = 1, .x = 2};
Q a5 = {.p = {1, 2}, .p.z = 3};
V a6 = {.k = 1, 2, 3};
V a7 = {.n = 1, .m = 2};
W a8 = {.u.f = 1.0f, .a[1] = 2};
Q b4 = {.p.y = 1, .p.y = 2};
W b5 = {.a = {1}, .a[2] = 5};
Q b6 = {.p = {.y = 1}, .p.x = 2};
V c1 = {.n = 1, .k = 2};
Q c3 = {.p = {1, 2}, .p.y = 5};
Q c4 = {.p = {.y = 1}, .p.y = 2};
U c5 = {.i = 1, .i = 2};
struct X { int a, b; } c6 = {.b = 1, .a = 2, .b = 3};
Pt c7 = {.y = 1, .x = 2, .z = 3};
Pt c8 = {.z = 1, .x = 2, .y = 3};
Pt c9 = {1, .z = 3};
Pt d1 = {.x = 1, 2, .y = 3};
int d2[4] = {[2] = 1, 5, [0] = 3};
int d3[] = {[3] = 1, [1] = 2};
static_assert(sizeof(d3) == 4 * sizeof(int));
