// [dcl.init.aggr]/16: braces may be elided in a designated-initializer-clause
// as an extension (clang `ext_designated_init_brace_elision`, -Wc99-designator):
// `.m = 1` initializes the first element of the aggregate member `m`, and the
// following non-designated clauses continue initializing it. This used to
// loop forever re-processing the designated clause.
//@ strict-diags
//@ diag: 21:23 warning "brace elision for designated initializer is a C99 extension"
//@ diag: 22:18 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 22:26 note "first non-designated initializer is here"
//@ diag: 22:23 warning "brace elision for designated initializer is a C99 extension"
//@ diag: 25:18 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 25:26 note "first non-designated initializer is here"
//@ diag: 25:23 warning "brace elision for designated initializer is a C99 extension"
//@ diag: 26:23 warning "brace elision for designated initializer is a C99 extension"
//@ diag: 28:18 warning "nested designators are a C99 extension"
//@ diag: 28:18 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 28:28 note "first non-designated initializer is here"
//@ diag: 28:25 warning "brace elision for designated initializer is a C99 extension"
//@ diag: 29:23 warning "brace elision for designated initializer is a C99 extension"
struct U { int m[2]; };
constexpr U u2 { .m = 1 };
constexpr U u3 { .m = 1, 2 };
struct I { int x, y; };
struct V { I s; int z; };
constexpr V v1 { .s = 1, 2, .z = 3 };
constexpr V v2 { .s = 1, .z = 3 };
struct W { U u; };
constexpr W w1 { .u.m = 1, 2 };
constexpr W w2 { .u = 1 };
static_assert(u2.m[0] == 1 && u2.m[1] == 0);
static_assert(u3.m[0] == 1 && u3.m[1] == 2);
static_assert(v1.s.x == 1 && v1.s.y == 2 && v1.z == 3);
static_assert(v2.s.x == 1 && v2.s.y == 0 && v2.z == 3);
static_assert(w1.u.m[0] == 1 && w1.u.m[1] == 2);
static_assert(w2.u.m[0] == 1 && w2.u.m[1] == 0);
