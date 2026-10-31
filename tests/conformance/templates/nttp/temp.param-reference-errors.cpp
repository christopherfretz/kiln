// [temp.arg.nontype]/2-3: a reference argument must not bind to a
// temporary or to an automatic variable, and the conversion must exist.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:3 error "conversion from 'int' to 'const int &' in converted constant expression would bind reference to a temporary"
//@ diag: 18:27 error "non-type template argument is not a constant expression"
//@ diag: 18:27 note "reference to 'loc' is not a constant expression"
//@ diag: 18:16 note "declared here"
//@ diag: 21:3 error "value of type 'const int' is not implicitly convertible to 'int &'"
//@ diag: 23:44 error "static assertion expression is not an integral constant expression"
//@ diag: 24:7 note "in instantiation of template class 'T<gi>' requested here"
//@ diag: 23:44 note "read of non-const variable 'gi' is not allowed in a constant expression"
//@ diag: 15:5 note "declared here"
struct A { int x; };
int gi = 3;
template <const int& r> struct R {};
R<5> r1;
void f() { int loc = 1; R<loc> r2; }
template <int& r> struct S {};
const int ci = 4;
S<ci> s1;
R<gi> r3;
template <int& r> struct T { static_assert(r == 0); };
T<gi> t;
