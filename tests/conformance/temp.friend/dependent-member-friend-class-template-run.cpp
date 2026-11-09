// [temp.friend]/5 in a class template: `template<class T> friend class
// C<T>::Member;` in B<U> befriends C<T>::Member of every C, for every
// specialization of B (B<T>, B<long> and B<T *> here), and the member
// function friend C<T>::f likewise. Output checked against clang 19 / 22.
//@ expect: run
//@ strict-diags
//@ diag: 16:41 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 17:39 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ stdout: 14 7
#include <cstdio>
template <class T> struct C {
  struct Member { static int get(); };
  static int f();
};
template <class U> class B {
  template <class T> friend class C<T>::Member;
  template <class T> friend int C<T>::f();
  static const int n = 7;
};
template <class T> int C<T>::Member::get() { return B<T>::n + B<long>::n; }
template <class T> int C<T>::f() { return B<T *>::n; }
int main() { std::printf("%d %d\n", C<int>::Member::get(), C<short>::f()); }
