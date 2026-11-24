// [temp.friend]/1, [class.access.general]/1: only the befriended
// specializations reach the private members of C<X>; It<X, false> and
// D::In<X, 3> are other specializations of the same templates (WP-3270).
// Reduced from real code.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:38 error "'v' is a private member of 'C<int>'"
//@ diag: 37:29 note "in instantiation of member function 'It<int, false>::get' requested here"
//@ diag: 24:7 note "implicitly declared private here"
//@ diag: 27:38 error "'v' is a private member of 'C<int *>'"
//@ diag: 38:31 note "in instantiation of member function 'It<int *, false>::get' requested here"
//@ diag: 24:7 note "implicitly declared private here"
//@ diag: 33:38 error "'v' is a private member of 'C<int>'"
//@ diag: 39:28 note "in instantiation of member function 'D::In<int, 3>::get' requested here"
//@ diag: 24:7 note "implicitly declared private here"
template <class T, bool B> struct It;
struct D {
  template <class U, int N> struct In;
};
template <class T> class C {
  friend It<T, true>;
  friend class It<T *, true>;
  friend struct D::In<T, 2>;
  int v = 1;
};
template <class T, bool B> struct It {
  static int get(C<T> &c) { return c.v; }
};
template <class T> struct It<T *, true> {
  static int get(C<T> &c) { return c.v; }
};
template <class U, int N> struct D::In {
  static int get(C<U> &c) { return c.v; }
};
int f(C<int> &a, C<int *> &b) {
  int ok = It<int, true>::get(a) + It<int *, true>::get(a) + D::In<int, 2>::get(a);
  int no1 = It<int, false>::get(a);
  int no2 = It<int *, false>::get(b);
  int no3 = D::In<int, 3>::get(a);
  return ok + no1 + no2 + no3;
}
