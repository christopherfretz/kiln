// [class.friend]/10, [temp.friend]/1, [temp.res.general]/3: in a class
// template, `friend int B::f(W<T>&);` befriends the member of the
// non-template class B whose type matches in each specialization; the
// match depends on T, so it is made when W<A> is instantiated, not when W
// is defined (WP-1110). clang and g++ print 42 43.
//@ expect: run
//@ stdout: 42 43
#include <cstdio>
template <class T> class W;
struct B { static int f(W<int> &w); static int f(W<long> &w); };
template <class T> class W {
  int secret = 42;
  friend int B::f(W<T> &);
};
int B::f(W<int> &w) { return w.secret; }
int B::f(W<long> &w) { return w.secret + 1; }
int main() {
  W<int> a; W<long> b;
  std::printf("%d %d\n", B::f(a), B::f(b));
}
