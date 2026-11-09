// [class.friend]/10, [temp.friend]/1, [temp.res.general]/3: a friend
// naming a member function whose declarator depends on the enclosing
// class template is matched when the class is instantiated, against the
// member with the substituted signature (WP-1110):
// - a member function template of a non-template class, own head and a
//   dependent parameter (`template <class U> friend int B::g(U, W<T>&)`);
// - a specifier whose arguments are not the enclosing template's own
//   parameter list (`B<T>::` inside `V<X, T>`), the qualifier resolved
//   from the substituted specifier;
// - a non-dependent friend in the same class (still matched at definition).
// Each friend's access is exercised after instantiation. clang and g++
// print 42 43 44 45.
//@ expect: run
//@ stdout: 42 43 44 45
#include <cstdio>
template <class T> class W;
template <class X, class T> class V;
struct B {
  template <class U> static int g(U, W<int> &w);
  template <class U> static int g(U, W<long> &w);
  static int h(int);
};
template <class T> struct C { static int f(V<char, T> &v); static int f(int); };
struct E { static int plain(W<int> &w); };
template <class T> class W {
  int secret = 42;
  template <class U> friend int B::g(U, W<T> &);
  friend int E::plain(W<int> &);
};
template <class X, class T> class V {
  int secret = 44;
  friend int C<T>::f(V<X, T> &);
};
template <class U> int B::g(U, W<int> &w) { return w.secret; }
template <class U> int B::g(U, W<long> &w) { return w.secret + 1; }
int B::h(int) { return 0; }
template <class T> int C<T>::f(V<char, T> &v) { return v.secret; }
template <class T> int C<T>::f(int) { return 0; }
int E::plain(W<int> &w) { return w.secret + 3; }
int main() {
  W<int> a; W<long> b; V<char, int> c;
  std::printf("%d %d %d %d\n", B::g(1, a), B::g('x', b), C<int>::f(c), E::plain(a));
}
