// [dcl.pre]/6 (C++17, N3928): static_assert without a message, at
// namespace, class and block scope and in templates. WP-646 feature probe.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 12:15 error "static assertion failed due to requirement 'sizeof(char) == 2'"
//@ diag: 12:28 note "expression evaluates to '1 == 2'"
static_assert(sizeof(int) >= 2);
struct S { static_assert(true); };
template <class T> void f() { static_assert(sizeof(T) > 1); }
void g() { static_assert(1 + 1 == 2); f<int>(); }
static_assert(sizeof(char) == 2);
