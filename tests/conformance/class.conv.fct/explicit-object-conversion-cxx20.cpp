// [dcl.fct]/3 (P0847): explicit object parameters, on a conversion function
// or anywhere else, are C++23; clang rejects them before C++23 and kiln
// does too (and then parses on as C++23).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 10:44 error "explicit object parameters are incompatible with C++ standards before C++2b"
//@ diag: 12:13 error "explicit object parameters are incompatible with C++ standards before C++2b"
//@ diag: 13:18 error "explicit object parameters are incompatible with C++ standards before C++2b"
struct S { int v=3; constexpr operator int(this const S& s){return s.v;} };
static_assert(int(S{})==3);
auto l = [](this auto& s) { return 1; };
struct A { int f(this A&); };
