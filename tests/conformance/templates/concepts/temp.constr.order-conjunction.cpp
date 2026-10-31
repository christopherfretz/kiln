//@ expect: compile-ok
// [temp.constr.order]/1: P && Q subsumes P.
template <class T> concept A = sizeof(T) >= 4;
template <class T> concept B = A<T> && __is_trivially_copyable(T);
template <A T> constexpr int f(T) { return 1; }
template <B T> constexpr int f(T) { return 2; }
static_assert(f(1) == 2);
struct NT {
  constexpr NT() {}
  constexpr NT(const NT &) {}
  int a = 0;
};
static_assert(f(NT{}) == 1);
