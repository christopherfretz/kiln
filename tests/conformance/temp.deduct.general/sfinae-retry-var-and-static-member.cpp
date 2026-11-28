// [temp.deduct.general]/8 (WP-3697): a function template specialization
// whose signature failed because a static data member or variable
// template was not yet defined is re-deduced once its definition
// appears.
//@ expect: compile-ok
template <int N> struct X {
  using t = int;
};

struct S {
  static const int n;
};
template <class T> auto g_member(T) -> typename X<T::n>::t;
long g_member(...);

static_assert(sizeof(g_member(S{})) == sizeof(long));
const int S::n = 1;
static_assert(sizeof(g_member(S{})) == sizeof(int));

template <class T> extern const int v;
template <class T> auto g_var(T) -> typename X<v<T>>::t;
long g_var(...);

static_assert(sizeof(g_var(0)) == sizeof(long));
template <class T> const int v = 1;
static_assert(sizeof(g_var(0)) == sizeof(int));
