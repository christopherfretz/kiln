export module default_targ_mod;
export namespace dm {
struct S {
  static int s() { return 1; }
};
struct U {
  static long s() { return 2; }
};
template <class T, int = sizeof(T::s())> int call() { return 0; }
inline int in_unit() { return call<S>(); }
} // namespace dm
