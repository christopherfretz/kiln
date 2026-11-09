// C11 6.5.1.1 (_Generic in C++, a clang extension): a type-dependent
// controlling operand (expression or type form) or a dependent
// association type leaves the selection dependent until instantiation,
// where it is checked and resolved; a value-dependent selected
// association stays value-dependent; selection works in a trailing
// return type, a default template argument and a requires-expression,
// where no match is a substitution failure.
//@ expect: compile-ok
//@ strict-diags
template <class T> constexpr int kind(T v) { return _Generic(v, int: 1, long: 2, double: 3, default: 0); }
static_assert(kind(1) == 1 && kind(1L) == 2 && kind(1.0) == 3 && kind('c') == 0);
template <class T, class U> constexpr int same() { return _Generic(T, U: 1, default: 0); }
static_assert(same<int, int>() == 1 && same<int, long>() == 0);
template <class T> constexpr int deptype(int x) { return _Generic(x, T: 1, default: 0); }
static_assert(deptype<int>(0) == 1 && deptype<char>(0) == 0);
template <int N> constexpr int vdep() { return _Generic(0, int: N * 2, default: -1); }
static_assert(vdep<21>() == 42);
template <class T> auto ret(T v) -> decltype(_Generic(v, int: 1L, default: 1.0)) { return 0; }
static_assert(__is_same(decltype(ret(1)), long) && __is_same(decltype(ret('c')), double));
template <class T, int K = _Generic(T, int: 1, default: 2)> constexpr int dflt() { return K; }
static_assert(dflt<int>() == 1 && dflt<short>() == 2);
template <class T> concept IsInt = requires(T t) { _Generic(t, int: 0); };
static_assert(IsInt<int> && !IsInt<long>);
template <class T> struct Box { T t; constexpr int k() const { return _Generic(t, T: 7); } };
static_assert(Box<float>{}.k() == 7);
template <class... Ts> constexpr int count = (_Generic(Ts, int: 1, default: 0) + ... + 0);
static_assert(count<int, char, int> == 2);
