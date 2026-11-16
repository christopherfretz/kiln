// [class.friend]/2, [temp.friend]/1: a friend declaration that names one
// overload befriends that function only. The sibling overloads stay
// non-friends whichever is declared first or instantiated first, through a
// member access or a qualified name of a static member (which used to be
// accessible once the friend had instantiated the member's definition), for
// members of a class template specialization, of a non-template class and
// for namespace-scope functions. A function of the pattern that the friend
// was instantiated from waits for its instantiations (`Q4<int>::f(int)` is
// the friend, `Q4<long>::f(int)` is not). The accepted side is
// friend-member-overload-access-ok.cpp. Found running GCC's C++ testsuite
// (WP-1513).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:35 error "'s' is a private member of 'V3<int>'"
//@ diag: 54:66 note "implicitly declared private here"
//@ diag: 72:57 error "'s' is a private member of 'V4<int>'"
//@ diag: 69:71 note "implicitly declared private here"
//@ diag: 40:59 error "'x' is a private member of 'V1<int>'"
//@ diag: 41:34 note "in instantiation of member function 'Q1<int>::f' requested here"
//@ diag: 35:7 note "implicitly declared private here"
//@ diag: 40:75 error "'s' is a private member of 'V1<int>'"
//@ diag: 36:14 note "implicitly declared private here"
//@ diag: 49:54 error "'s' is a private member of 'V2<int>'"
//@ diag: 51:35 note "in instantiation of member function 'Q2<int>::f' requested here"
//@ diag: 46:14 note "implicitly declared private here"
//@ diag: 65:56 error "'s' is a private member of 'V5<int>'"
//@ diag: 66:33 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 62:70 note "implicitly declared private here"
//@ diag: 71:56 error "'s' is a private member of 'V4<int>'"
//@ diag: 73:55 note "in instantiation of member function 'Q4<long>::f' requested here"
//@ diag: 69:71 note "implicitly declared private here"
template <class T> struct Q1 { void f(int); void f(long); };
template <class T> class V1 {
  friend void Q1<T>::f(int);
  int x = 0;
  static int s;
};
template <class T> int V1<T>::s = 0;
template <class T> void Q1<T>::f(int) { V1<T> v; (void)v.x; (void)V1<T>::s; }
template <class T> void Q1<T>::f(long) { V1<T> v; (void)v.x; (void)V1<T>::s; }
void g1() { Q1<int> q; q.f(1); q.f(1L); }

template <class T> struct Q2 { void f(int); void f(long); };
template <class T> class V2 {
  friend void Q2<T>::f(long);
  static int s;
};
template <class T> int V2<T>::s = 0;
template <class T> void Q2<T>::f(int) { (void)V2<T>::s; }
template <class T> void Q2<T>::f(long) { (void)V2<T>::s; }
void g2() { Q2<int> q; q.f(1L); q.f(1); }

struct Q3 { void f(int); void f(long); };
template <class T> class V3 { friend void Q3::f(int); static int s; };
template <class T> int V3<T>::s = 0;
void Q3::f(int) { (void)V3<int>::s; }
void Q3::f(long) { (void)V3<int>::s; }

template <class T> class V5;
template <class T> void h(V5<T>&, int);
template <class T> void h(V5<T>&, long);
template <class T> class V5 { friend void h<T>(V5&, int); static int s; };
template <class T> int V5<T>::s = 0;
template <class T> void h(V5<T>&, int) { (void)V5<T>::s; }
template <class T> void h(V5<T>&, long) { (void)V5<T>::s; }
void g5() { V5<int> v; h(v, 1); h(v, 1L); }

template <class T> struct Q4 { void f(int); void f(long); };
template <class T> class V4 { friend void Q4<int>::f(int); static int s; };
template <class T> int V4<T>::s = 0;
template <class T> void Q4<T>::f(int) { (void)V4<int>::s; }
template <class T> void Q4<T>::f(long) { (void)V4<int>::s; }
void g4() { Q4<int> q; q.f(1); q.f(1L); Q4<long> r; r.f(1); }
