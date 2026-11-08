// [temp.func.order]/3 with P3310R5: against a template template parameter's
// template-id (the unique type of partial ordering) `A`'s extra arguments
// bind as defaults of the deduced template, but a template with too few
// parameters does not match, so `f(TT<int, int>)` is more specialized than
// `f(TT<int>)` (clang 19 calls it ambiguous) and `g(TT<X, int>)` no longer
// loses to `g(TT<int>)` (p0522-partial-order-unique-ttp-clang19.cpp; WP-812).
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
template <class T, class U = int> struct A {};
template <template <class> class TT> constexpr int f(TT<int>) { return 1; }
template <template <class, class> class TT> constexpr int f(TT<int, int>) { return 2; }
template <template <class> class TT> constexpr int g2(TT<int>) { return 1; }
template <class X, template <class, class> class TT> constexpr int g2(TT<int, X>) { return 2; }
template <template <class> class TT> constexpr int k(TT<int>) { return 1; }
template <class X> constexpr int k(X) { return 2; }
static_assert(f(A<int>()) == 2);
static_assert(g2(A<int>()) == 2);
static_assert(k(A<int>()) == 1);
