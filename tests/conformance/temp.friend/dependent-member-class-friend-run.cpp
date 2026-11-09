// [temp.friend]/5: `template<class T> friend class A<T>::Member;` befriends
// the member class `Member` of every specialization of A, so
// A<int>::Member::get may read B's private member. clang 19 and 22 accept
// it (they warn -Wunsupported-friend and turn off access control for B);
// kiln used to reject the declaration as "redefinition of 'Member' as
// different kind of symbol" and, since WP-1054, accepts the declaration
// but still denies the access.
//@ expect: run
//@ xfail(kiln): WP-1054 follow-up, a friend member class of a dependent specialization (`A<T>::Member`) grants no access
//@ stdout: 7
#include <cstdio>
class B;
template <class T> struct A {
  struct Member { static int get(const B &b); };
};
class B {
  template <class T> friend class A<T>::Member;
  int n = 7;
};
template <class T> int A<T>::Member::get(const B &b) { return b.n; }
int main() { std::printf("%d\n", A<int>::Member::get(B())); }
