//@ expect: ast
//@ oracle: clang
// [temp.class]/1: a static member function of a specialization, named with a
// qualified-id, is instantiated on use.
template <class T> struct Maker {
  static T zero() { return T(); }
  static T one() { return T(1); }
};
long z = Maker<long>::zero();
