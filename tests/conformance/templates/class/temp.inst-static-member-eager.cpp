//@ expect: ast
//@ oracle: clang
// [temp.inst]/4: a non-inline static data member's in-class initializer is
// instantiated with the class; an inline one waits for a use.
template <class T> struct S {
  static const int b = 2;
  static inline int a = 1;
  static int d;
};
static_assert(S<int>::b == 2);
