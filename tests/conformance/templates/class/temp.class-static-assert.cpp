//@ expect: ast
//@ oracle: clang
// [temp.inst]/3, [dcl.pre]/10: a static_assert member is instantiated (and
// evaluated) with the class; a dependent one is checked per specialization.
template <class T, int N> struct Arr {
  static_assert(N > 0, "positive");
  static_assert(sizeof(T) >= 1);
  T data[N];
};
int sz = sizeof(Arr<short, 3>);
