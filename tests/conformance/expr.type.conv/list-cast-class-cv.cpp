// [expr.type.conv]/2, [expr.prop]/[basic.lval]/7: `T{...}` for a
// cv-qualified class type `T` is a prvalue of type cv `T`; the cv-qualifiers
// are kept (only a non-class prvalue drops them). This is what makes
// `std::forward_like<int>(CU{})` yield `const U&&` (libc++
// forward_like.pass.cpp). Aggregate, constructor and template forms.
// Matches clang 19.
//@ expect: run
//@ stdout: 7
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;

struct U {};
struct V { int v; constexpr V() : v(1) {} constexpr V(int x) : v(x) {} };
struct W { int a; };
using CU = const U;
using CV = const V;
using CW = const W;
using CI = const int;

template <class X> X&& fwd(X&& x) { return static_cast<X&&>(x); }
template <class X> constexpr bool is_const_rref(X&&) { return same<X, const U>; }

static_assert(same<decltype(CU{}), const U>);
static_assert(same<decltype(CV{}), const V>);
static_assert(same<decltype(CV{2}), const V>);
static_assert(same<decltype(CW{1}), const W>);
static_assert(same<decltype(CI{1}), int>);
static_assert(same<decltype(fwd(CU{})), const U&&>);
static_assert(same<decltype(fwd(CW{3})), const W&&>);
static_assert(is_const_rref(CU{}));

template <class T> constexpr bool dep() { return same<decltype(T{}), T>; }
static_assert(dep<const U>());
static_assert(dep<const V>());
static_assert(!dep<const int>());

extern "C" int printf(const char*, ...);
int main() {
  int a = fwd(CW{3}).a;
  printf("%d\n", a + fwd(CV{4}).v);
}
