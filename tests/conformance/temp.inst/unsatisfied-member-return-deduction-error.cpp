// [expr.prim.id.general]/4: a qualified call of a static member function
// with a placeholder return type whose constraints are not satisfied is
// "invalid reference ... constraints not satisfied", and its body (with
// its error) is not instantiated (WP-1497).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:20 error "invalid reference to function 'r': constraints not satisfied"
//@ diag: 11:28 note "because 'false' evaluated to false"
template <class T> struct A {
  static auto r() requires false { return T::fail; }
};
void g() { A<int>::r(); }
