// [stmt.if]/2: a variable's initializer in a discarded statement is part
// of it, so what the initializer names is not odr-used and no function
// template specialization is instantiated for it ([temp.inst]/8), for an
// automatic or a static local alike. clang 21 and later keep the
// initializer's evaluation context discarded (`InDiscardedStatement`);
// clang 20 and earlier odr-use there (discarded-statement-var-init.cpp).
//@ expect: run
//@ requires: clang>=22
//@ exit: 3
template <class T> inline constexpr bool never = false;
template <class T> int f(T) {
  static_assert(never<T>, "f instantiated");
  return 0;
}
template <class T> struct W {
  W() {}
  W(const W &) { static_assert(never<T>, "W copy"); }
};
int main() {
  if constexpr (false) {
    int x = f(1);
    static int y = f(2L);
    int (*p)(char) = f;
    auto q = &f<short>;
    W<int> w;
    W<int> c = w;
    return x + y + p('a') + q(0);
  }
  return 3;
}
