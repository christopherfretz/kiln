// [class.mem]/1, [expr.sizeof]/2: a class with an invalid data member (an
// unknown type, an incomplete type, a negative array bound or bit-field
// width, a duplicate member), a base of one, or a member of one, is
// invalid; clang then gives it size and alignment 1 so no follow-on
// layout diagnostic fires. An invalid static data member or member
// function does not make the class invalid.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ args: -ferror-limit=0
//@ xfail(kiln): an invalid class keeps its partial layout: the size-1 rule belongs in kiln_abi's Layouter (the oven's sizeof reads it) and the class is not marked invalid at its closing brace (WP-392 follow-up)
//@ diag: 25:26 error "unknown type name 'undeclared_t'"
//@ diag: 26:15 error "static assertion failed due to requirement 'sizeof(A) == 1'"
//@ diag: 26:25 note "expression evaluates to '8 == 1'"
//@ diag: 27:19 error "unknown type name 'undeclared_t'"
//@ diag: 28:15 error "static assertion failed due to requirement 'sizeof(B) == 1'"
//@ diag: 28:25 note "expression evaluates to '8 == 1'"
//@ diag: 29:25 error "'b' declared as an array with a negative size"
//@ diag: 31:24 error "field has incomplete type 'void'"
//@ diag: 33:23 error "bit-field 'b' has negative width (-1)"
//@ diag: 35:19 error "unknown type name 'undeclared_t'"
//@ diag: 41:23 error "duplicate member 'a'"
//@ diag: 41:16 note "previous declaration is here"
//@ diag: 43:18 error "unknown type name 'undeclared_t'"
struct A { int a; static undeclared_t s; int c; };
static_assert(sizeof(A) == 1);
struct B { int a; undeclared_t f(); int c; };
static_assert(sizeof(B) == 1);
struct C { int a; int b[-1]; };
static_assert(sizeof(C) == 1);
struct D { int a; void b; };
static_assert(sizeof(D) == 1);
struct E { int a; int b : -1; };
static_assert(sizeof(E) == 1);
struct F { int a; undeclared_t b; };
static_assert(alignof(F) == 1);
struct G : F { int x; };
static_assert(sizeof(G) == 1);
struct H { F f; int x; };
static_assert(sizeof(H) == 1);
struct I { int a; int a; };
static_assert(sizeof(I) == 1);
union J { int a; undeclared_t b; };
static_assert(sizeof(J) == 1);
constexpr F fv{};
constexpr int k = sizeof(F) + 1;
static_assert(k == 2);
