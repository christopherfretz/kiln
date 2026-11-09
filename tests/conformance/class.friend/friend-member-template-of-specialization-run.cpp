// [class.friend]/10, [temp.friend]/1: `template <class U> friend int
// B<T>::g(U, W<T>&);` in a class template befriends the member template g
// of whichever specialization `B<A>` names (primary or explicit). clang and
// g++ print 42 43.
//@ expect: run
//@ xfail(kiln): WP-1071 follow-up, a friend member template of a dependent specialization is matched at definition ("does not match any declaration in 'B<T>'")
//@ stdout: 42 43
#include <cstdio>
template <class T> class W;
template <class T> struct B { template <class U> static int g(U, W<T> &); static int g(int); };
template <> struct B<long> { template <class U> static int g(U, W<long> &); static int g(int); };
template <class T> class W {
  int secret = 42;
  template <class U> friend int B<T>::g(U, W<T> &);
};
template <class T> template <class U> int B<T>::g(U, W<T> &w) { return w.secret; }
template <class U> int B<long>::g(U, W<long> &w) { return w.secret + 1; }
int main() {
  W<int> a; W<long> b;
  std::printf("%d %d\n", B<int>::g(1, a), B<long>::g(1, b));
}
