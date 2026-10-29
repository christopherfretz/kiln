//@ expect: ast
//@ oracle: clang
// [temp.inst]/8: a constexpr member function is instantiated when it is
// needed for constant evaluation.
template <class T> struct Sq {
  static constexpr T of(T x) { return x * x; }
};
static_assert(Sq<int>::of(7) == 49);
