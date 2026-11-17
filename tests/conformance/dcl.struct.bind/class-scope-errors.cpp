// [class.mem.general]/1: a member-declarator is never a structured binding
// declaration. One error at the `[` per declaration, with or without
// `static` or `inline`; the rest of the declaration is skipped (its names
// are not reported as undeclared) and the class stays usable.
// Found running GCC's C++ testsuite.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:17 error "declaration not permitted in this context"
//@ diag: 16:24 error "declaration not permitted in this context"
//@ diag: 17:31 error "declaration not permitted in this context"
//@ diag: 18:36 error "declaration not permitted in this context"
struct Q { int a, b; };
Q q();
struct S { auto [a, b] = q(); int c = 0; };
struct T { static auto [a, b] = q(); int c; };
struct U { inline static auto [a, b] {q()}; static constexpr int c = 1; };
template <class X> struct V { auto [a, b] = X(); int c = 2; };
int x = S().c + T().c + U::c + V<Q>().c;
