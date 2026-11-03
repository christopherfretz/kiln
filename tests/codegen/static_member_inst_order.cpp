// [temp.inst]/4, /5: implicitly instantiated static data members of class
// templates, out-of-line definitions and inline members' in-class
// initializers alike, are emitted in clang's order: the end-of-TU drain
// instantiates them in the order their uses queued them, so the
// `__cxx_global_var_init` functions and the globals follow that order
// (WP-186).
int g(int);
template <class T> struct A {
  static int x;
  static int y;
  static int z;
};
template <class T> int A<T>::y = g(2);
template <class T> int A<T>::x = g(1);
template <class T> int A<T>::z = g(3);
template <class T> struct B {
  static inline int i = g(4);
  static int j;
};
template <class T> int B<T>::j = g(5);
template <class T> int tf() { return A<T>::z + B<T>::j; }
int use() { return A<int>::x + A<int>::y + tf<int>() + B<int>::i + A<char>::y; }
int use2() { return A<long>::z; }
