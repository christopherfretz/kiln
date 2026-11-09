// [temp.friend]/5: `template<class T> friend class A<T>::Member;` befriends
// the member class `Member` of every specialization of A, so
// A<int>::Member::get may read B's private member. clang 19 and 22 accept
// it (they warn -Wunsupported-friend and turn off access control for B);
// kiln used to reject the declaration as "redefinition of 'Member' as
// different kind of symbol", then (WP-1054) accepted it but denied the
// access; WP-1072 models the rule.
//@ expect: run
//@ strict-diags
//@ diag: 18:41 warning "for friend class declaration is not supported; turning off access control for 'B'"
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
