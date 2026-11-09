// [temp.friend]/5 names one member of the specializations the friend's
// head deduces from, and nothing else: a sibling member class (A<T>::Other),
// a non-friend member function (A<T>::g), a member function whose type
// differs from the substituted friend's (A<long>::k(int) against k(long)),
// the member of a specialization the specifier does not match (A<int> for
// A<T *>; the error is reported in the instantiation), the same-named member
// of another class template (O2) and of a non-template class (Plain) are all
// denied. clang turns off access control for B instead (-Wunsupported-friend)
// and accepts the TU (an accepts-invalid; g++ 14 denies these).
//@ skip(clang): clang turns off access control for the befriending class
//@ expect: compile-fail
//@ strict-diags
//@ diag: 46:43 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 47:39 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 51:64 error "'n' is a private member of 'B'"
//@ diag: 48:7 note "implicitly declared private here"
//@ diag: 53:55 error "'n' is a private member of 'B'"
//@ diag: 48:7 note "implicitly declared private here"
//@ diag: 55:66 error "'n' is a private member of 'B'"
//@ diag: 48:7 note "implicitly declared private here"
//@ diag: 56:47 error "'n' is a private member of 'B'"
//@ diag: 48:7 note "implicitly declared private here"
//@ diag: 50:65 error "'n' is a private member of 'B'"
//@ diag: 59:26 note "in instantiation of member function 'A<int>::Member::get' requested here"
//@ diag: 48:7 note "implicitly declared private here"
//@ diag: 52:60 error "'n' is a private member of 'B'"
//@ diag: 60:61 note "in instantiation of member function 'A<long>::k' requested here"
//@ diag: 48:7 note "implicitly declared private here"
class B;
template <class T> struct A {
  struct Member { static int get(const B &b); };
  struct Other { static int get(const B &b); };
  static int k(int, const B &b);
  static int g(const B &b);
};
template <class T> struct A<T *> {
  struct Member { static int get(const B &b); };
};
template <class T> struct O2 {
  struct Member { static int get(const B &b); };
};
struct Plain {
  struct Member { static int get(const B &b); };
};
class B {
  template <class T> friend class A<T *>::Member;
  template <class T> friend int A<T>::k(T, const B &);
  int n = 7;
};
template <class T> int A<T>::Member::get(const B &b) { return b.n; }
template <class T> int A<T>::Other::get(const B &b) { return b.n; }
template <class T> int A<T>::k(int, const B &b) { return b.n; }
template <class T> int A<T>::g(const B &b) { return b.n; }
template <class T> int A<T *>::Member::get(const B &b) { return b.n; }
template <class T> int O2<T>::Member::get(const B &b) { return b.n; }
int Plain::Member::get(const B &b) { return b.n; }
int main() {
  B b;
  return A<int>::Member::get(b) + A<int *>::Member::get(b) +
         A<int>::Other::get(b) + A<int>::k(0, b) + A<long>::k(0, b) +
         A<int>::g(b) + O2<int>::Member::get(b) + Plain::Member::get(b);
}
