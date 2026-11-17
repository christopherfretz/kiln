// [dcl.init.general]/16.6.2.2 (C++20, P0960): `(U)x` initializes a union's
// FIRST member from a parenthesized list, so a C-style cast from the first
// member's type is valid in C++20; a later member's type converts to the
// first member, never selects it (C++ has no GNU cast to union, clang
// `CheckCXXCStyleCast`). WP-1587.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ diag: 12:10 warning "implicit conversion from 'double' to 'int' changes value from 2.5 to 2"
union U { int i; double d; };
U a = (U)1;
U b = (U)2.5;
static_assert(sizeof(U) == sizeof(double));
