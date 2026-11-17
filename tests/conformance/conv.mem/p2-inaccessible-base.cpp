// [conv.mem]/2: converting "pointer to member of B" to "pointer to member of
// D" is ill-formed if B is an inaccessible base of D: implicitly, for a
// member function pointer, through several levels, in a constant
// expression and in the candidate overload resolution picks (an
// inaccessible base does not remove the candidate).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:13 error "cannot cast private base class 'B' to 'D'"
//@ diag: 22:12 note "declared private here"
//@ diag: 26:24 error "cannot cast private base class 'B' to 'D'"
//@ diag: 22:12 note "declared private here"
//@ diag: 27:14 error "cannot cast protected base class 'B' to 'P'"
//@ diag: 23:12 note "declared protected here"
//@ diag: 28:16 error "cannot cast private base class 'B' to 'E'"
//@ diag: 22:12 note "declared private here"
//@ diag: 29:23 error "cannot cast private base class 'B' to 'D'"
//@ diag: 22:12 note "declared private here"
//@ diag: 32:17 error "cannot cast private base class 'B' to 'D'"
//@ diag: 22:12 note "declared private here"
struct B { int m; int f() const; };
struct D : private B {};
struct P : protected B {};
struct E : D {};
int D::*a = &B::m;
int (D::*af)() const = &B::f;
int P::*ap = &B::m;
int E::*deep = &B::m;
constexpr int D::*c = &B::m;
void ov(int D::*);
void ov(long);
void use() { ov(&B::m); }
