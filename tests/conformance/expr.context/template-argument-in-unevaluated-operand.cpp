// [expr.context]/1, [expr.const]/22: a template argument is a constant
// expression even inside an unevaluated operand, so the constructor it
// calls is used and defined there (`sizeof(f<(T(), 0)>())`); one that is
// not constexpr makes the argument invalid.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 21:21 error "no matching function for call to 'skip'"
//@ diag: 11:27 note "candidate template ignored: invalid explicitly-specified argument for 1st template parameter"

template <int, class T> T skip();

struct A { int m = -1; };
template <class T> constexpr unsigned size_of() { return sizeof(skip<(T(), 0), T>()); }
static_assert(size_of<A>() == sizeof(A), "");

struct B { int m = -1; };
static_assert(sizeof(skip<(B(), 0), B>()) == sizeof(B), "");

struct N { int m; N() : m(1) {} };
unsigned n = sizeof(skip<(N(), 0), N>());
