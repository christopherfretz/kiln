// [expr.const]/5.10 (C++23, P2280R4): an id-expression naming a reference
// whose referent is unknown may appear in a constant expression when its
// value is not needed, as in `size(arr)` for a reference-to-array parameter.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ requires: clang>=22
template <class T, unsigned long N> constexpr unsigned long size(T (&)[N]) { return N; }
void f(int (&a)[4]) { static_assert(size(a) == 4); }
