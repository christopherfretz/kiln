//@ expect: ast
//@ oracle: clang
// [temp.inst]/4: the initializer of an inline static data member is
// instantiated only when the member is used.
template <class T> struct Traits {
  static constexpr int size = sizeof(T);
  static constexpr int align = alignof(T);
};
static_assert(Traits<int>::size == 4);
