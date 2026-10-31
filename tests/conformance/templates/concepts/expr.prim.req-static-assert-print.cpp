//@ expect: compile-fail
//@ strict-diags
// A failed static_assert of a requires-expression prints the expression as
// clang's StmtPrinter does, with no satisfaction notes (WP-109).
//@ diag: 13:15 error "static assertion failed due to requirement 'requires { requires sizeof(int) == 2; }'"
//@ diag: 14:15 error "static assertion failed due to requirement 'requires (A a) { { a.f() } noexcept; }'"
//@ diag: 15:15 error "static assertion failed due to requirement 'requires (A a) { a.x; requires sizeof (a) == 1; }': msg"
//@ diag: 16:23 error "static assertion failed due to requirement 'requires { requires false; }'"
//@ diag: 17:15 error "static assertion failed due to requirement 'requires (A a, int i) { { a.x } -> Same<long>; i + 1; }'"
//@ diag: 18:15 error "static assertion failed due to requirement 'requires { int; requires false; }'"
template <class T, class U> concept Same = __is_same(T, U);
struct A { int x; void f(); using type = int; };
static_assert(requires { requires sizeof(int) == 2; });
static_assert(requires(A a) { { a.f() } noexcept; });
static_assert(requires(A a) { a.x; requires sizeof(a) == 1; }, "msg");
static_assert(true && requires { requires false; });
static_assert(requires(A a, int i) { { a.x } -> Same<long>; i + 1; });
static_assert(requires { typename A::type; requires false; });
