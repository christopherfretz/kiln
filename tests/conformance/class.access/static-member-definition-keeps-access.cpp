// [class.access]/1, [temp.inst]/4: the instantiated definition of a static
// data member of a class template is a redeclaration of the member and keeps
// its access. Once one function had used `V<int>::x` (instantiating that
// definition), later qualified names `V<int>::x` found the definition and
// were accessible everywhere, also where no friend grants access. Found
// running GCC's C++ testsuite (WP-1513).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:17 error "'y' is a private member of 'V<int>'"
//@ diag: 29:14 note "implicitly declared private here"
//@ diag: 43:17 error "'y' is a private member of 'V<int>'"
//@ diag: 29:14 note "implicitly declared private here"
//@ diag: 36:43 error "'x' is a private member of 'V<int>'"
//@ diag: 39:3 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 28:14 note "implicitly declared private here"
//@ diag: 36:58 error "'e' is a private member of 'V<int>'"
//@ diag: 30:12 note "implicitly declared private here"
//@ diag: 36:74 error "'h' is a private member of 'V<int>'"
//@ diag: 31:15 note "implicitly declared private here"
//@ diag: 37:43 error "'x' is a private member of 'V<int>'"
//@ diag: 40:3 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 28:14 note "implicitly declared private here"
//@ diag: 37:58 error "'e' is a private member of 'V<int>'"
//@ diag: 30:12 note "implicitly declared private here"
//@ diag: 37:74 error "'h' is a private member of 'V<int>'"
//@ diag: 35:31 note "implicitly declared private here"
template <class T> class V {
  static int x;
  static int y;
  enum E { e };
  static void h();
};
template <class T> int V<T>::x = 0;
template <class T> int V<T>::y = 0;
template <class T> void V<T>::h() {}
template <class T> void f() { (void)V<T>::x; (void)V<T>::e; (void)&V<T>::h; }
template <class T> void g() { (void)V<T>::x; (void)V<T>::e; (void)&V<T>::h; }
void u() {
  f<int>();
  g<int>();
}
int a = V<int>::y;
int b = V<int>::y;
