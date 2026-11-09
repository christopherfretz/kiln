// [temp.spec.partial.general]/9.2, [temp.deduct.type]/20: a primary
// parameter typed by another one (`template<class T, T>`) deduces that
// type from the partial specialization's argument as written, so
// `template<bool B> V<int, B>` is not more specialized (`T` would be both
// `int` and `bool`). Clang 19 converted the argument to `int` first and
// accepted it, which `-fclang-compat=19` keeps
// (temp.spec.partial.match/narrowing-nttp-dependent-type-clang19.cpp);
// clang 22 and kiln reject it (WP-1105).
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:26 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 15:30 note "template is declared here"
template <class T, T> struct V { static constexpr int v = 0; };
template <bool B> struct V<int, B> { static constexpr int v = 1; };
template <class T, T> struct W { static constexpr int v = 0; };
template <int *P> struct W<int *, P> { static constexpr int v = 1; };
static_assert(W<int *, nullptr>::v == 1);
