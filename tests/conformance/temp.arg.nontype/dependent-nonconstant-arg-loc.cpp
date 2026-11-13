// [temp.arg.nontype]/2: a non-type argument that is not a constant
// expression once its template is instantiated is reported at the
// argument (clang 19/22), at its substituted expression: not at locations
// the parser recorded for another template-id (22:13) nor the template
// name (21:38), as kiln did before WP-1362 (found by WP-1361).
// The "in instantiation of" notes follow each error, as clang orders them.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:77 error "non-type template argument is not a constant expression"
//@ diag: 20:19 note "in instantiation of function template specialization 'O<int>::f<X>' requested here"
//@ diag: 19:77 note "read of non-const variable 'v' is not allowed in a constant expression"
//@ diag: 18:24 note "declared here"
//@ diag: 21:40 error "non-type template argument is not a constant expression"
//@ diag: 22:11 note "in instantiation of function template specialization 'g<X>' requested here"
//@ diag: 21:40 note "read of non-const variable 'v' is not allowed in a constant expression"
//@ diag: 18:24 note "declared here"
template <long N> struct I {};
struct X { static long v; };
template <class T> struct O { template <class U> static long f() { return I<U::v + 0>{}, 0; } };
long u4 = O<int>::f<X>();
template <class U> long g() { return I<U::v + 0>{}, 0; }
long u5 = g<X>();
