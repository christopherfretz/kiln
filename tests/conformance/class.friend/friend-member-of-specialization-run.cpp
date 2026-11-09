// [class.friend]/10, [temp.friend]/1, [temp.dep.type]/6: `friend int
// B<T>::f(W<T>&);` in a class template befriends the member of whichever
// specialization `B<A>` names. In each W<A> the friend is declared as a
// member of B<A>, so it redeclares B<int>::f (kiln declared a namespace
// function and granted no access) and, for the explicit specialization
// B<long> with an overloaded `f`, the overload whose type matches, found by
// looking the name up again (kiln reported "not yet implemented"; WP-1071).
//@ expect: run
//@ stdout: 42 43 44
#include <cstdio>
template <class T> class W;
template <class T> struct B { static int f(W<T> &w); static int f(int); };
template <> struct B<long> { static int f(W<long> &w); static int f(int); static int f(double); };
template <class T> struct B<T *> { int f(W<T *> &w); template <class U> int f(U); };
template <class T> class W {
  int secret = 42;
  friend int B<T>::f(W<T> &);
};
template <class T> int B<T>::f(W<T> &w) { return w.secret; }
template <class T> int B<T>::f(int) { return 0; }
int B<long>::f(W<long> &w) { return w.secret + 1; }
template <class T> int B<T *>::f(W<T *> &w) { return w.secret + 2; }
int main() {
  W<int> a; W<long> b; W<char *> c;
  std::printf("%d %d %d\n", B<int>::f(a), B<long>::f(b), B<char *>().f(c));
}
