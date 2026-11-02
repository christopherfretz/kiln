// [over.match.class.deduct]/1: an error forming the implicit deduction guides of
// a member template of a specialization (substituting the enclosing arguments
// into a constructor) is followed by clang's "while building implicit deduction
// guide first needed here" note at the template name of the placeholder that
// first needed them: in a declaration, a functional cast, a new-expression, and
// through an alias-qualified name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:48 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 34:13 note "while building implicit deduction guide first needed here"
//@ diag: 34:16 error "no viable constructor or deduction guide for deduction of template arguments of 'Outer<int>::In'"
//@ diag: 32:28 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 32:28 note "implicit deduction guide declared as 'template <class U> In(Outer<int>::In<U>) -> Outer<int>::In<U>'"
//@ diag: 32:48 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 35:31 note "while building implicit deduction guide first needed here"
//@ diag: 35:18 error "no viable constructor or deduction guide for deduction of template arguments of 'Outer<char>::In'"
//@ diag: 32:28 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 32:28 note "implicit deduction guide declared as 'template <class U> In(Outer<char>::In<U>) -> Outer<char>::In<U>'"
//@ diag: 32:48 error "type 'short' cannot be used prior to '::' because it has no members"
//@ diag: 36:37 note "while building implicit deduction guide first needed here"
//@ diag: 36:23 error "no viable constructor or deduction guide for deduction of template arguments of 'Outer<short>::In'"
//@ diag: 32:28 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 32:28 note "implicit deduction guide declared as 'template <class U> In(Outer<short>::In<U>) -> Outer<short>::In<U>'"
//@ diag: 32:48 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 38:15 note "while building implicit deduction guide first needed here"
//@ diag: 38:18 error "no viable constructor or deduction guide for deduction of template arguments of 'O::In'"
//@ diag: 32:28 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 32:28 note "implicit deduction guide declared as 'template <class U> In(Outer<long>::In<U>) -> Outer<long>::In<U>'"
//@ requires: clang>=22
// clang 22 spelling of building-guides-note.cpp (WP-377).
template<class T> struct Outer {
  template<class U> struct In { In(U, typename T::type); };
};
Outer<int>::In i(1, 2);
void f() { (void)Outer<char>::In(1, 2); }
void g() { (void) new Outer<short>::In(1, 2); }
using O = Outer<long>;
void h() { O::In x{1, 2}; }
