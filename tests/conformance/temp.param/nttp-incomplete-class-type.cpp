// [temp.param]/6, [temp.arg.nontype]: a non-type template parameter of
// class type needs a complete (structural) type, also when the type comes
// from substituting a prior template argument (clang's
// `RequireStructuralType`: "non-type template parameter has incomplete
// type", the forward declaration noted).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:12 error "non-type template parameter has incomplete type 'S'"
//@ diag: 13:8 note "forward declaration of 'S'"
//@ diag: 15:21 error "non-type template parameter has incomplete type 'S'"
//@ diag: 16:1 note "while substituting prior template arguments into non-type template parameter 't' [with T = S]"
//@ diag: 13:8 note "forward declaration of 'S'"
struct S;
template<S s> struct A;
template<class T, T t> struct C {};
C<S, 0> *c;
struct Done {};
template<Done d> struct B {};
