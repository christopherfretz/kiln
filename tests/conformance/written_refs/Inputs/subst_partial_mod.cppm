// The module unit of subst-partial-module-image.cpp and
// subst-partial-module-replay.cpp: two partial specializations match
// `W<S, int *, int>`, instantiated in the unit.
export module subst_partial_mod;
export namespace sm {
struct S {
  using type = int;
};
template <class T, class U, class = void> struct W {
  static const int v = 0;
};
template <class T, class U> struct W<T, U *, typename T::type> {
  static const int v = 1;
};
template <class T> struct W<T, int *, typename T::type> {
  static const int v = 2;
};
inline int in_unit() { return W<S, int *, int>::v; }
} // namespace sm
