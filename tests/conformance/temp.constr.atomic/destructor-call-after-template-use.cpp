// [temp.constr.atomic]/3, [temp.dep]/1: an implicitly declared member of a
// non-template class is not in a dependent context even when the class is
// first used inside a template definition (`S x;` in a function template
// body declares `S::~S()`), so a later `v.~T()` requirement with `T = S`
// is not dependent after substitution (libc++'s
// `__cpp17_destructible`, pstl.merge.pass.cpp,
// pstl.exception_handling.pass.cpp).
//@ expect: compile-ok
//@ strict-diags
template <class T> concept D = requires(T v) { v.~T(); };
struct S {};
struct U { U(const U &); };
template <class X> void h() {
  S x;
  U *u = nullptr;
  (void)x;
  (void)u;
}
static_assert(D<S>);
static_assert(D<int>);
template <class T> struct W { T t; };
template <class X> void k() { W<S> w; (void)w; }
static_assert(D<W<S>>);
