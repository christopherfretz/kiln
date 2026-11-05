// [expr.prim.id.general]/4 (C++11, N2253): a non-static data member may be
// named in an unevaluated operand without an object. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++11
//@ strict-diags
struct S { int i; double d[4]; struct In { char c[3]; } in; };
static_assert(sizeof(S::i) == sizeof(int), "sizeof member");
static_assert(sizeof(S::d) == 4 * sizeof(double), "array member");
static_assert(sizeof(S::in.c) == 3, "nested member");
static_assert(alignof(decltype(S::d)) == alignof(double), "decltype member");
template <class T> struct Sz { static const unsigned long v = sizeof(T::i); };
static_assert(Sz<S>::v == sizeof(int), "dependent member");
struct U { int m; static const unsigned long n = sizeof(m); };
static_assert(U::n == sizeof(int), "inside its class");
