// [temp.inst]/8, [expr.const]/20 (C++20, P0859R0 / CWG1581): a constexpr
// function is instantiated only when it is needed for constant
// evaluation; a call in an unevaluated decltype operand does not need it.
// WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
template <class T> constexpr int f() { return T::missing; }
using R = decltype(f<int>());
static_assert(__is_same(R, int));
template <class T> constexpr int g() { return sizeof(T); }
static_assert(g<long>() == sizeof(long));
template <int N> struct A {};
using B = decltype(A<g<char>()>{});
