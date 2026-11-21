export module lcown;
// Local classes of an inline and a constexpr member function of an
// exported class, of a lambda in an exported inline variable, and of a
// function template the module unit instantiates too.
export struct Host {
  int k;
  inline int viaMember() const {
    struct R { int a; int get() const { return a + 100; } };
    return R{k}.get();
  }
  constexpr int viaConstexpr() const {
    struct C { int a; int get() const { return a + 200; } };
    return C{k}.get();
  }
};
export inline auto viaLambda = [](int x) {
  struct V { int a; int get() const { return a + 300; } };
  return V{x}.get();
};
export template <class T> inline T twice(T x) {
  struct P { T a; T go() const { return a + a; } };
  return P{x}.go();
}
export int moduleSide() { return twice(21); }
