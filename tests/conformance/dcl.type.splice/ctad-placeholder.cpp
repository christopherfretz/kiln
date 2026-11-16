// [dcl.type.splice]/2 (P2996): a splice-type-specifier designating a
// class template (or an alias template) is a placeholder for a deduced
// class type ([dcl.type.class.deduct]): `[: ^^X :] y{13};` deduces as
// `X y{13};` does, in a template too. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ oracle: gcc
template <class T> struct X { T v; constexpr X(T t) : v(t) {} };
template <class T> struct P { T a, b; };
constexpr auto rx = ^^X;
constexpr [: rx :] y{13};
static_assert(__is_same(decltype(y), const X<int>));
static_assert(y.v == 13);

template <decltype(^^int) R> constexpr auto g() {
  typename [: R :] q{2.5};
  return q.v;
}
static_assert(g<^^X>() == 2.5);
constexpr [: ^^P :] p{1L, 2L};
static_assert(__is_same(decltype(p), const P<long>));
