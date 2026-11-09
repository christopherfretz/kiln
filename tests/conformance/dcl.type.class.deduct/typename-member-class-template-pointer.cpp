// [dcl.type.class.deduct]/1-2: a placeholder for a deduced class type is
// allowed only as a variable's whole declared type; 'typename T::X *p'
// with X a member class template is ill-formed when instantiated. WP-958.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:51 error "typename specifier refers to class template member in 'A'; argument deduction not allowed here"
//@ diag: 15:14 note "in instantiation of function template specialization 'bad<A>' requested here"
//@ diag: 12:32 note "template is declared here"

struct A {
  template <typename T> struct X { constexpr X(T) {} };
};
template <typename T> void bad(T t) { typename T::X *p = 0; }
void use() { bad(A{}); }
