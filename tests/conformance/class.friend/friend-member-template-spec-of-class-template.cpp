// [temp.friend]/1, [class.friend]/2: `friend void Q<T>::f<int>(int);` in a
// class template befriends the specialization `Q<int>::f<int>` of the
// instantiated class's member template (not one of the pattern's), and no
// other specialization of it; `friend Q<T>::Q(int);` befriends that
// constructor and not its sibling `Q<int>::Q(long)`. Found running GCC's
// C++ testsuite (WP-1513).
//@ skip(clang): clang 22 rejects a friend naming a specialization of a dependent class's member template, and befriends no constructor of a dependent class
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:73 error "'x' is a private member of 'V<int>'"
//@ diag: 19:39 note "in instantiation of function template specialization 'Q<int>::f<long>' requested here"
//@ diag: 17:64 note "implicitly declared private here"
//@ diag: 25:48 error "'s' is a private member of 'W<int>'"
//@ diag: 26:14 note "in instantiation of member function 'R<int>::R' requested here"
//@ diag: 22:62 note "implicitly declared private here"
template <class T> struct Q { template <class U> int f(U); };
template <class T> class V { friend int Q<T>::f<int>(int); int x = 0; };
template <class T> template <class U> int Q<T>::f(U) { V<T> v; return v.x; }
int g() { Q<int> q; return q.f(1) + q.f(1L); }

template <class T> struct R { R(int); R(long); };
template <class T> class W { friend R<T>::R(int); static int s; };
template <class T> int W<T>::s = 0;
template <class T> R<T>::R(int) { (void)W<T>::s; }
template <class T> R<T>::R(long) { (void)W<T>::s; }
R<int> a(1), b(1L);
