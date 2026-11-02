// [basic.scope.pdecl]/9: the locus of a template parameter is immediately
// after its complete template-parameter, so a name in its own default
// argument finds the enclosing scope's entity, not the parameter itself
// (libc++ `tuple`'s allocator-extended copy constructor:
// `template <class...> class _And = _And`). Type, non-type and template
// template parameters, at namespace scope and in a member template of a
// class template; a later parameter does see an earlier one. Verified
// against clang 19.
//@ expect: compile-ok
//@ strict-diags
struct T { static constexpr int v = 1; };
constexpr int N = 2;
template <class... Ts> struct And { static constexpr int value = 4; };
template <bool B, class R = void> struct enable_if {};
template <class R> struct enable_if<true, R> { using type = R; };

template <class T = T> struct X { static constexpr int v = T::v; };
static_assert(X<>::v == 1);
template <int N = N> struct Y { static constexpr int v = N; };
static_assert(Y<>::v == 2);
template <template <class...> class And = And> struct Z { static constexpr int v = And<int>::value; };
static_assert(Z<>::v == 4);

template <class U> struct W {
  template <class T = T, int N = N, template <class...> class And = And>
  static constexpr int f() { return T::v + N + And<U>::value; }
  template <class A, template <class...> class And = And,
            typename enable_if<And<U>::value == 4, int>::type = 0>
  W(int, A) {}
  W() = default;
};
static_assert(W<int>::f() == 7);
W<int> w(1, 2);

template <class P, class Q = P> struct Later { using type = Q; };
static_assert(__is_same(Later<int>::type, int));
