// [temp.deduct.general]/8 (P4149R1, Croydon DR; CWG2672): the body of a
// lambda-expression in a function template's signature is not in the
// immediate context of the deduction, so an error in it is a hard error even
// though a fallback overload exists.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:53 error "member reference base type 'int' is not a structure or union"
//@ diag: 10:45 note "while substituting into a lambda expression here"
//@ diag: 12:10 note "while substituting deduced template arguments into function template 'h' [with T = int]"
template <class T> auto h(T) -> decltype([] { T t; t.nope(); return 0; }());
long h(...);
auto r = h(0);
