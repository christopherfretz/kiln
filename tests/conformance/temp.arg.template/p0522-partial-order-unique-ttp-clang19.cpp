// [temp.func.order]/3, [temp.deduct.type]/9 with CWG2398 as clang 19
// deduces it: against a template template parameter's template-id (the
// unique type of partial ordering) `P`'s extra arguments are ignored and
// `A`'s extra ones bind as defaults, so `g(TT<int>)` is more specialized
// than `g(TT<X, int>)` and `g2(TT<int, X>)` than `g2(TT<int>)` (the clang 22
// results are p0522-partial-order-unique-ttp-clang22.cpp; WP-812).
//@ requires: clang<22
//@ expect: compile-ok
//@ strict-diags
template <class T, class U = int> struct A {};
template <template <class> class TT> constexpr int g(TT<int>) { return 1; }
template <class X, template <class, class> class TT> constexpr int g(TT<X, int>) { return 2; }
template <template <class> class TT> constexpr int g2(TT<int>) { return 1; }
template <class X, template <class, class> class TT> constexpr int g2(TT<int, X>) { return 2; }
template <template <class> class TT> constexpr int k(TT<int>) { return 1; }
template <class X> constexpr int k(X) { return 2; }
static_assert(g(A<int>()) == 1);
static_assert(g2(A<int>()) == 2);
static_assert(k(A<int>()) == 1);
