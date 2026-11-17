// GNU: a constexpr union whose flexible array member is initialized: the
// member is the active one, so reading another member is not a constant
// expression; its elements lie past the union and cannot be reached in a
// constant expression (array-to-pointer decay of an array member without
// known bound); a nested initialization is ill-formed. Notes as clang 22.
// The accepted side is run-union-constexpr-elements.cpp.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-gnu-flexible-array-initializer -Wno-c99-designator -Wno-flexible-array-extensions
//@ diag: 21:15 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 21:19 note "read of member 'a' of union with active member 'b' is not allowed in a constant expression"
//@ diag: 22:22 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 22:26 note "array-to-pointer decay of array member without known bound is not supported"
//@ diag: 23:22 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 23:27 note "array-to-pointer decay of array member without known bound is not supported"
//@ diag: 25:24 error "initialization of flexible array member is not allowed"
//@ diag: 19:23 note "initialized flexible array member 'b' is here"
union U1 { int a; int b[]; };
constexpr U1 u1 = {.b = {1, 2, 3, 4, 5, 6}};
constexpr int r = u1.a;
constexpr const int *p = u1.b;
constexpr const int *q = &u1.b[5];
struct H { U1 u; };
constexpr H h = {{.b = {1, 2}}};
