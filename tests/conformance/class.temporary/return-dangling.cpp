// [stmt.return]/6 (P2748R5, C++26): a return statement that binds the
// returned reference to a temporary expression is ill-formed; [class.temporary]/6
// looks through parentheses, no-op and derived-to-base conversions and the
// right operand of a comma. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:26 error "returning reference to local temporary object"
//@ diag: 18:24 error "returning reference to local temporary object"
//@ diag: 19:21 error "returning reference to local temporary object"
//@ diag: 20:29 error "returning reference to local temporary object"
//@ diag: 21:27 error "returning reference to local temporary object"
int g;
struct S { int a; };
struct D : S {};
S mk();
D mkd();
const int &f1() { return 1; }
const S &f2() { return mk(); }
int &&f3() { return (2); }
const S &f4(int x) { return g = x, mkd(); }
const long &f5() { return 4; }
const S &ok(const S &s) { return s; }
int &&mv();
int &&ok2() { return mv(); }
