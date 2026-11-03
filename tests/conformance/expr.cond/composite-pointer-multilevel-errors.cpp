// [expr.type]/4.6, [conv.qual]/2: pointers whose innermost types differ
// are not similar and have no composite pointer type; base and derived
// class pointers combine only at the first level (/4.5).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:16 error "incompatible operand types ('int **' and 'long **')"
//@ diag: 14:16 error "incompatible operand types ('B **' and 'D **')"
//@ diag: 15:23 error "comparison of distinct pointer types ('int **' and 'long **')"
//@ diag: 16:24 error "comparison of distinct pointer types ('B **' and 'D **')"
int **pp; long **lpp;
struct B {}; struct D : B {};
B **bpp; D **dpp;
auto e1 = true ? pp : lpp;
auto e2 = true ? bpp : dpp;
bool e3() { return pp == lpp; }
bool e4() { return bpp == dpp; }
B *bp; const D *dp;
static_assert(__is_same(decltype(true ? bp : dp), const B *));
