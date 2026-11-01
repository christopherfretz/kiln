// [temp.mem], [over.match.class.deduct]/1: the guide of a constructor
// template of a member class template, named as `Outer<int>::H`, has the
// member's template parameters followed by the constructor template's,
// with the enclosing argument substituted; a guide parameter with a
// default argument prints it as `<null expr>`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "no viable constructor or deduction guide for deduction of template arguments of 'Outer<int>::H'"
//@ diag: 14:50 note "candidate template ignored: couldn't infer template argument 'U'"
//@ diag: 14:50 note "implicit deduction guide declared as 'template <class U, class V> H(V, int, V = <null expr>) -> H<U>'"
//@ diag: 14:28 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 14:28 note "implicit deduction guide declared as 'template <class U> H(H<U>) -> H<U>'"
template<class T> struct Outer {
  template<class U> struct H { template<class V> H(V, T, V = V()); };
};
Outer<int>::H h(1.0, 2);
