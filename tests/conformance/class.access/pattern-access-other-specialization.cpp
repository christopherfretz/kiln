// [class.access.general]/2, [temp.inst]/11: in a member of a class
// template, a private member of another specialization of the same
// template is accessible only when the specialization being instantiated
// is that one. The pattern's check is redone in each instantiation: in
// A<int> naming A<int>'s private members is fine; B<long> naming B<int>'s
// is an error reported when B<long>::f is instantiated. Found running
// GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:26 error "'g' is a private member of 'B<int>'"
//@ diag: 26:17 note "in instantiation of member function 'B<long>::f' requested here"
//@ diag: 24:8 note "declared private here"
template<class T> struct A {
  bool f() { A<int> a; return a.g(); }
  static void h() { A<int>::s(); }
private:
  bool g() const { return true; }
  static void s() {}
};
template struct A<int>;
template<class T> struct B {
  void f() { B<int> b; b.g(); }
private:
  void g() {}
};
template struct B<long>;
