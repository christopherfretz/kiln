//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:15 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 48:15 note "pointer to 's' is not a constant expression"
//@ diag: 48:15 note "address of non-static constexpr variable 's' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 49:15 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 49:15 note "reference to 'r' is not a constant expression"
//@ diag: 49:15 note "address of non-static constexpr variable 'r' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 50:15 error "constexpr variable 'u' must be initialized by a constant expression"
//@ diag: 50:15 note "pointer to subobject of 'u' is not a constant expression"
//@ diag: 50:15 note "address of non-static constexpr variable 'u' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 51:15 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 51:15 note "pointer to subobject of 'a' is not a constant expression"
//@ diag: 51:15 note "address of non-static constexpr variable 'a' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 53:18 error "constexpr variable 'l' must be initialized by a constant expression"
//@ diag: 53:18 note "reference to 'k' is not a constant expression"
//@ diag: 52:7 note "declared here"
//@ diag: 54:24 error "constexpr variable 'pk' must be initialized by a constant expression"
//@ diag: 54:24 note "pointer to 'k' is not a constant expression"
//@ diag: 52:7 note "declared here"
//@ diag: 55:38 error "constexpr variable 'pp' must be initialized by a constant expression"
//@ diag: 55:38 note "pointer to 'parm' is not a constant expression"
//@ diag: 47:22 note "declared here"
//@ diag: 58:53 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 59:15 note "in instantiation of function template specialization 't<S>' requested here"
//@ diag: 58:53 note "pointer to 'x' is not a constant expression"
//@ diag: 58:53 note "address of non-static constexpr variable 'x' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
//@ diag: 60:33 error "constexpr variable 's' must be initialized by a constant expression"
//@ diag: 60:33 note "pointer to 's' is not a constant expression"
//@ diag: 60:33 note "address of non-static constexpr variable 's' may differ on each invocation of the enclosing function; add 'static' to give it a constant address"
// [expr.const]/22 before C++26 (permitted results): a constant expression
// may not hold the address of an object of automatic storage duration, the
// constexpr local being initialized included. Each kind of value that points
// into a constexpr local or to another automatic object is rejected with
// clang's notes: a pointer or reference member to the object itself, a
// pointer to its own member, array elements pointing to each other, a closure
// capturing a local by reference, a pointer to a local or a parameter, in a
// template and in an immediate function. Namespace-scope and `static
// constexpr` objects keep a constant address and are accepted. From C++26 the
// automatic ones are well-formed (constexpr-local-self-pointer.cpp).
// Reduced from a find running GCC's C++ testsuite (WP-1680).
struct S { const S *self = this; int v = 1; };
struct R { const R &self = *this; int v = 2; };
struct U { int v = 3; const int *p = &v; };
struct N { const N *next; int v; };
constexpr bool f(int parm) {
  constexpr S s{};
  constexpr R r{};
  constexpr U u{};
  constexpr N a[2] = {{&a[1], 4}, {&a[0], 5}};
  int k = parm;
  constexpr auto l = [&k] { return k; };
  constexpr const int *pk = &k;
  constexpr struct { const int *p; } pp{&parm};
  return s.self == &s;
}
template <class T> constexpr bool t() { constexpr T x{}; return x.self == &x; }
template bool t<S>();
consteval int c() { constexpr S s{}; return s.v; }
constexpr S g{};
static_assert(g.self == &g);
constexpr bool h() { static constexpr S st{}; return st.self == &st; }
static_assert(h());
