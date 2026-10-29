// [expr.eq]/4: only == and != compare member pointers ([expr.rel]/2 orders
// object pointers only); both operands must have a composite pointer type
// ([expr.type]/4), and a non-null integer is not a null pointer constant
// ([conv.mem]/1). [conv.mem]/2: no conversion through a virtual base.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:14 error "invalid operands to binary expression ('int B::*' and 'int B::*')"
//@ diag: 26:14 error "comparison of distinct pointer types ('int B::*' and 'int O::*')"
//@ diag: 27:14 error "comparison of distinct pointer types ('int B::*' and 'long B::*')"
//@ diag: 28:14 error "invalid operands to binary expression ('int B::*' and 'int')"
//@ diag: 29:14 error "comparison of distinct pointer types ('int B::*' and 'int *')"
//@ diag: 30:14 error "invalid operands to binary expression ('int B::*' and 'std::nullptr_t')"
//@ diag: 31:14 error "comparison of distinct pointer types ('void (B::*)()' and 'int B::*')"
//@ diag: 32:17 error "conversion from pointer to member of class 'B' to pointer to member of class 'V' via virtual base 'B' is not allowed"
//@ diag: 32:14 error "comparison of distinct pointer types ('int V::*' and 'int B::*')"
//@ diag: 33:14 error "invalid operands to binary expression ('int B::*' and 'int')"
//@ diag: 34:14 error "invalid operands to binary expression ('int *' and 'int B::*')"
//@ diag: 35:14 error "invalid operands to binary expression ('int B::*' and 'int')"
struct B { int x; int w; void f(); };
struct D : B { int y; };
struct O { int z; };
struct V : virtual B {};
int B::*pb = &B::x, B::*qb = &B::w; int O::*po = &O::z; long B::*pl = nullptr;
void (B::*pf)() = &B::f; int *ip = nullptr; int V::*pv = nullptr; int i = 0;
bool b1 = pb < qb;
bool b2 = pb == po;
bool b3 = pb == pl;
bool b4 = pb == 1;
bool b5 = pb == ip;
bool b6 = pb >= nullptr;
bool b7 = pf == pb;
bool b8 = pv == pb;
bool b9 = pb <= 0;
bool c1 = ip > pb;
bool c2 = pb != i;
