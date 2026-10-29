//@ expect: ast
//@ oracle: clang
// [temp.inst]/3: member typedef and alias declarations are instantiated with
// the class; the substituted type is visible through the specialization.
template <class T> struct Traits {
  typedef T *pointer;
  using ref = const T &;
  pointer p;
};
Traits<int>::pointer gp;
int sz = sizeof(Traits<char>::ref);
