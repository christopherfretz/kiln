// [temp.deduct.general]/8, [expr.prim.req.general]/5: the only
// declarations local to a template that are substituted in an immediate
// context are function and requires-expression parameters. One whose type
// fails to substitute is a substitution failure, and so is every later
// use of it in the same signature or requirement (decltype(a), a + b):
// no diagnostic, the candidate is dropped or the requirement is false.
// A function body (and the local declarations in it) is never an
// immediate context (WP-316; see temp.inst/failed-local-declaration-uses.cpp).
//@ expect: compile-ok
struct Y { using type = int; };
template <class T> constexpr bool r = requires(typename T::type a, decltype(a) b) { a + b; };
static_assert(!r<int> && r<Y>);
template <class T> constexpr bool s = requires(T t) { requires requires(typename T::type a) { a; t; }; };
static_assert(!s<int> && s<Y>);
template <class T> constexpr int f(T, typename T::type a, decltype(a) = 0) { return 1; }
template <class T> constexpr int f(T, ...) { return 2; }
static_assert(f(1, 2) == 2 && f(Y{}, 2) == 1);
template <class T> auto g(T) { return [](auto u) -> decltype(typename decltype(u)::type{}, 1) { return 1; }; }
using L = decltype(g(1));
template <class U> constexpr bool h = requires(L l) { l(U{}); };
static_assert(!h<int> && h<Y>);
