//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:15 error "static assertion failed"
//@ diag: 23:15 note "because 'int' does not satisfy 'HasType'"
//@ diag: 19:58 note "because 'typename T::type' would be invalid: type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 24:15 error "static assertion failed"
//@ diag: 24:15 note "because 'A' does not satisfy 'NoThrow'"
//@ diag: 20:64 note "because 't.f()' may throw an exception"
//@ diag: 25:15 error "static assertion failed"
//@ diag: 25:15 note "because 'A' does not satisfy 'DotDot'"
//@ diag: 21:56 note "because 't.x.y' would be invalid: member reference base type 'int' is not a structure or union"
//@ diag: 26:15 error "static assertion failed"
//@ diag: 26:15 note "because 'A' does not satisfy 'Arrow'"
//@ diag: 22:53 note "because 't->x' would be invalid: member reference type 'A' is not a pointer; did you mean to use '.'?"
// [expr.prim.req]: where clang places a failed requirement's note: a
// type-requirement at the type, a noexcept requirement at `noexcept`, a
// member access after substitution at its `.` or `->`.
struct A { int x; void f(); };
template <class T> concept HasType = requires { typename T::type; };
template <class T> concept NoThrow = requires(T t) { { t.f() } noexcept; };
template <class T> concept DotDot = requires(T t) { t.x.y; };
template <class T> concept Arrow = requires(T t) { t->x; };
static_assert(HasType<int>);
static_assert(NoThrow<A>);
static_assert(DotDot<A>);
static_assert(Arrow<A>);
