// [over.match.ref]/1.1, [dcl.init.ref]/5.4: a non-const lvalue reference
// from a class whose conversion functions only yield const lvalues:
// clang reports the failed overload resolution with its candidates.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:9 error "no viable conversion from 'D' to 'double'"
//@ diag: 11:12 note "candidate function"
//@ diag: 14:9 error "no viable conversion from 'D2' to 'double'"
//@ diag: 13:13 note "candidate function"
//@ diag: 13:45 note "candidate function"
struct D { operator const double&() const; };
double& r = D();
struct D2 { operator const double&() const; operator const int&() const; };
double& r2 = D2();
