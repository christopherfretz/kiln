// [dcl.fct]/5, [temp.deduct.general]/7: a parameter declared as an array
// is adjusted to a pointer, but the bound it was declared with is still
// substituted (clang checks it through the parameter's declared type,
// its `DecayedType`): a bound that is ill-formed for the deduced or
// explicit arguments is a substitution failure. Here for a parameter of
// a function type inside another declarator, a function template's own
// parameter, and a value-dependent bound that becomes negative.
// Reduced from real code.
//@ expect: compile-ok
struct X {};
constexpr int f(int) { return 0; }

template <class T> constexpr int k(T, void (*)(int[sizeof(f(T()))])) { return 1; }
constexpr int k(...) { return 2; }
static_assert(k(X(), nullptr) == 2);
static_assert(k(1, nullptr) == 1);

template <class T> constexpr int m(T, int a[sizeof(f(T()))]) { return 1; }
constexpr int m(...) { return 2; }
static_assert(m(X(), nullptr) == 2);
static_assert(m(1, nullptr) == 1);

// An instantiation-dependent bound with a known value, adjusted.
template <class T> constexpr int n(T, int a[sizeof(sizeof(f(T())))]) { return 1; }
constexpr int n(...) { return 2; }
static_assert(n(X(), nullptr) == 2);
static_assert(n(1, nullptr) == 1);

template <int N> constexpr int v(int a[N]) { return 1; }
template <int N> constexpr int v(...) { return 2; }
static_assert(v<-1>(nullptr) == 2);
static_assert(v<2>(nullptr) == 1);

// The adjusted parameter type is still the pointer.
template <class T> constexpr bool is_ptr(int a[sizeof(f(T()))]) {
  return __is_same(decltype(a), int *);
}
static_assert(is_ptr<int>(nullptr));
static_assert(__is_same(decltype(k<int>), int(int, void (*)(int *))));
