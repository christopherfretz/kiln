// [temp.friend]/5: a friend declaration with its own template head whose
// nested-name-specifier names a specialization of a class template
// befriends that member of every specialization the head deduces from:
// a member class (with its constructor's mem-initializers and member
// function bodies), a nested member class (`A<T>::X::Y`), a member
// function (its type substituted and compared), a member function
// template, an explicit specialization's member and a partial
// specialization's (`A<T &>`); a partial specialization without the
// member (`A<T *>`) instantiates fine. clang 19 and 22 agree on the output
// (they warn -Wunsupported-friend at the befriended name, as kiln does,
// with the clang 19 / 22 wording pinned in dependent-member-friend-warning*).
//@ expect: run
//@ strict-diags
//@ diag: 40:41 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 41:44 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 42:39 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 43:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ stdout: 55 14 21 28 35 42 7
#include <cstdio>
class B;
template <class T> struct A {
  struct Member {
    int m;
    int k;
    explicit Member(const B &b);
    int get(const B &b) const;
  };
  struct X { struct Y { static int get(const B &b); }; };
  static int f(const B &b);
  template <class V> static int h(const B &b);
};
template <> struct A<char> {
  struct Member { static int get(const B &b); };
};
template <class T> struct A<T *> {};
template <class T> struct A<T &> {
  struct Member { static int get(const B &b); };
};
class B {
  template <class T> friend class A<T>::Member;
  template <class T> friend class A<T>::X::Y;
  template <class T> friend int A<T>::f(const B &);
  template <class T> template <class V> friend int A<T>::h(const B &);
  int n = 7;
  static const int seed = 40;
};
template <class T> A<T>::Member::Member(const B &b) : m(b.n), k(B::seed + 1) {}
template <class T> int A<T>::Member::get(const B &b) const { return b.n + m + k; }
template <class T> int A<T>::X::Y::get(const B &b) { return b.n * 2; }
template <class T> int A<T>::f(const B &b) { return b.n * 3; }
template <class T> template <class V> int A<T>::h(const B &b) { return b.n * 4; }
int A<char>::Member::get(const B &b) { return b.n * 5; }
template <class T> int A<T &>::Member::get(const B &b) { return b.n * 6; }
int main() {
  B b;
  A<int *> unused;
  (void)unused;
  std::printf("%d %d %d %d %d %d %d\n", A<int>::Member(b).get(b),
              A<long>::X::Y::get(b), A<int>::f(b), A<int>::h<char>(b),
              A<char>::Member::get(b), A<int &>::Member::get(b),
              A<double>::Member(b).m);
}
