// [over.match.oper]/3, [over.match.best]/3: a unary operator expression
// whose user-declared candidates tie is ambiguous; the diagnostic names
// the one operand type (clang `err_ovl_ambiguous_oper_unary`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "use of overloaded operator '-' is ambiguous (operand type 'A')"
//@ diag: 10:6 note "candidate function"
//@ diag: 11:6 note "candidate function"
struct A {};
void operator-(A);
void operator-(const A &);
void f(A a) { -a; }
