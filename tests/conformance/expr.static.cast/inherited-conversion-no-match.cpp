// [expr.static.cast]/4, [class.conv.fct]/9, [namespace.udecl]/1: a class
// with a conversion function only through a base, or through a
// using-declaration, has conversion functions, so a static_cast none of
// them makes viable is "no matching conversion" with the candidate, not
// "without a conversion operator" (which only a class with none gets).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:10 error "cannot convert 'E' to 'int' without a conversion operator"
//@ diag: 18:10 error "no matching conversion for static_cast from 'QD' to 'int'"
//@ diag: 14:12 note "candidate function"
//@ diag: 19:10 error "no matching conversion for static_cast from 'QP' to 'int'"
//@ diag: 14:12 note "candidate function"
struct E {};
struct Q { operator long*() const; };
struct QD : Q {};
struct QP : private Q { using Q::operator long*; };
int a1 = static_cast<int>(E());
int a3 = static_cast<int>(QD());
int a4 = static_cast<int>(QP());
