// [temp.dep.type]/1, [class.friend]/3: a member class of a class template
// reached through a specialization with non-dependent arguments
// (`O<int>::M::N`, also via an alias template) is not dependent, even before
// the specialization is instantiated, inside a template or not; and
// `friend T::X;` / `friend Outer::value_type;` in a class template names a
// type without `typename` (the libstdc++ `chunk_view` shape). Regression
// tests for WP-1084.
//@ expect: run
//@ strict-diags
//@ stdout: 3 4 8 5 6
#include <cstdio>
template <class T> struct O { struct M { struct N; }; };
template <class T> struct O<T>::M::N { T v = 3; };
template <class T> using A = O<T>;

template <class V> struct CV {
  struct Outer { using value_type = struct Val; };
  class Inner;
};
struct Val;
template <class V> class CV<V>::Inner {
  friend Outer::value_type;
  static int secret() { return 5; }
};
struct Val {
  static int peek() { return CV<int>::Inner::secret(); }
};

template <class T> struct P {
  friend T::X;
  static int hidden() { return 6; }
};
struct S { struct X { static int get() { return P<S>::hidden(); } }; };

template <class T> int f() {
  O<int>::M::N n;
  A<short>::M::N m;
  m.v = 4;
  std::printf("%d %d %zu ", n.v, int(m.v), sizeof(O<long>::M::N));
  return 0;
}
int main() {
  f<char>();
  std::printf("%d %d\n", Val::peek(), S::X::get());
}
