// [temp.deduct.guide]/3, [temp.inst]/3: a member deduction guide of a
// class template is instantiated with the class; a guide whose
// parameter type is invalid for the specialization is diagnosed then
// (`typename T::type` with `T = int`), and the guide does not take part
// in deduction.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:14 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 15:8 note "in instantiation of template class 'A<int>' requested here"
template <class T> struct A {
  template <class U> struct B { B(U); };
  B(typename T::type) -> B<T>;
};
A<int> a;
