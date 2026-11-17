// [expr.static.cast]/2,11: a static_cast from a base to a derived class
// pointer or reference needs an accessible base: clang reports it as a
// cast from the base ("cannot cast private base class 'B' to 'D'"); a
// C-style cast ignores the access ([expr.cast]/4).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:21 error "cannot cast private base class 'B' to 'D'"
//@ diag: 14:12 note "declared private here"
//@ diag: 17:21 error "cannot cast private base class 'B' to 'D'"
//@ diag: 14:12 note "declared private here"
//@ diag: 18:21 error "cannot cast protected base class 'B' to 'P'"
//@ diag: 15:12 note "declared protected here"
struct B { int m; };
struct D : private B {};
struct P : protected B {};
D *p(B *b) { return static_cast<D *>(b); }
D &r(B &b) { return static_cast<D &>(b); }
P *q(B *b) { return static_cast<P *>(b); }
D *c(B *b) { return (D *)b; }
D &cr(B &b) { return (D &)b; }
