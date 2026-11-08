// [temp.names]/1, [temp.arg.general] (P2996R13): a splice that only
// starts a template-argument is an ordinary operand: `[: r :] + 1`, a
// splice as a nested-name-specifier (`[: r :]::type`, `[: r :]::v`), a
// parenthesized splice, and `typename [: r :]` are all valid arguments.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
template <class T> struct X { using type = T; };
template <int N> struct Y { static constexpr int value = N; };
struct S {
  using type = int;
  static constexpr int v = 2;
};
constexpr int k = 3;

static_assert(Y<[: ^^k :] + 1>::value == 4);
static_assert(Y<([: ^^k :])>::value == 3);
static_assert(Y<[: ^^S :]::v>::value == 2);
static_assert(__is_same(X<[: ^^S :]::type>::type, int));
static_assert(__is_same(X<typename [: ^^int :]>::type, int));
static_assert(__is_same(X<const typename [: ^^int :]>::type, const int));

template <class T> struct Z {
  static constexpr int a = Y<[: ^^T :]::v * 2>::value;
  using t = X<typename [: ^^T :]::type>::type;
};
static_assert(Z<S>::a == 4);
static_assert(__is_same(Z<S>::t, int));
