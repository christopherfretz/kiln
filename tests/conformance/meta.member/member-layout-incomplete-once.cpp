// [meta.member]/2, /4: the class of a member-layout builtin's argument must
// be complete. A wrapper template (as std::is_corresponding_member) reports
// that once, in its instantiation: the call stays a `bool` expression, so
// the specialization is valid and later calls of it are not diagnosed again
// (no "no matching function"). Found running GCC's C++ testsuite (WP-1497).
//@ skip(clang): clang 19/22 do not have the builtins
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 15:122 error "incomplete type 'B' where a complete type is required"
//@ diag: 18:20 note "in instantiation of function template specialization 'cm<A, B>' requested here"
//@ diag: 13:8 note "forward declaration of 'B'"
struct B;
struct A { int a; };
template <class S1, class S2> constexpr bool cm(int S1::*m1, int S2::*m2) { return __builtin_is_corresponding_member(m1, m2); }
template <class S> constexpr bool pic(int S::*m) { return __builtin_is_pointer_interconvertible_with_class(m); }
constexpr int B::*n = nullptr;
constexpr bool x = cm(&A::a, n);
void f(int B::*m) { cm(&A::a, m); cm(&A::a, m); }
bool g(int A::*m) { return pic(m); }
