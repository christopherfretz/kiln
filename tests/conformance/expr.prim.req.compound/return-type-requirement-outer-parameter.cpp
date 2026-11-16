// [expr.prim.req.compound]/1, [temp.constr.decl]: a return-type
// requirement whose type-constraint names an enclosing template's
// parameter is checked in each specialization of the member template.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ args: -std=c++20
//@ diag: 26:10 error "constraints not satisfied for class template 'B' [with $0 = <>]"
//@ diag: 19:34 note "because 'is_same<int, bool>' evaluated to false"
//@ diag: 16:47 note "because '__is_same(int, bool)' evaluated to false"
//@ diag: 29:3 error "no matching function for call to 'f'"
//@ diag: 23:15 note "candidate template ignored: constraints not satisfied [with $0 = <>]"
//@ diag: 22:34 note "because 'is_same<int, bool>' evaluated to false"
//@ diag: 16:47 note "because '__is_same(int, bool)' evaluated to false"
template <class T, class U> concept is_same = __is_same(T, U);
template <class T> struct A {
  template <class...>
    requires requires { { 0 } -> is_same<T>; }
  struct B {};
  template <class...>
    requires requires { { 1 } -> is_same<T>; }
  static void f() {}
};
A<int>::B<> a1;
A<bool>::B<> a2;
void g() {
  A<int>::f();
  A<bool>::f();
}
