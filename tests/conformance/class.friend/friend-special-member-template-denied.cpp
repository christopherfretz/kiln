// [temp.friend]/1: a friend constructor template or conversion function
// template befriends that member template only, not the non-template
// constructor or conversion function of the class (sibling of
// friend-special-member-template-run.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:24 error "'x' is a private member of 'Y'"
//@ diag: 20:14 note "implicitly declared private here"
//@ diag: 26:30 error "'x' is a private member of 'Y'"
//@ diag: 20:14 note "implicitly declared private here"
struct Z {
  template <class U> Z(U);
  Z(int *);
  template <class U> operator U*();
  operator int();
};
class Y {
  template <class U> friend Z::Z(U);
  template <class U> friend Z::operator U*();
  static int x;
};
int Y::x = 3;
template <class U> Z::Z(U) { (void)Y::x; }
Z::Z(int *) { (void)Y::x; }
template <class U> Z::operator U*() { (void)Y::x; return 0; }
Z::operator int() { (void)Y::x; return 0; }
void g() { Z a(1), c((int *)0); (void)(char *)a; (void)(int)a; }
