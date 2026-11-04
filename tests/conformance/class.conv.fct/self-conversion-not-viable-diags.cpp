// [class.conv.fct]/1: a conversion function to the class itself (or to
// a base) is never used, so where no constructor is viable the
// initialization fails rather than calling it; the conversion function
// is noted as a plain candidate, after the constructors (clang
// `ovl_fail_trivial_conversion`, sorted as an arity mismatch since it has
// no parameter for the one argument). -Wclass-conversion is reported for
// the non-template class only, not for a member of an instantiated class
// template (`W<int>`). Matches clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:24 warning "conversion function converting 'X' to itself will never be used"
//@ diag: 28:3 error "no matching constructor for initialization of 'X'"
//@ diag: 24:17 note "candidate constructor not viable: 1st argument ('const X') would lose const qualifier"
//@ diag: 24:12 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 24:24 note "candidate function"
//@ diag: 30:3 error "no matching constructor for initialization of 'X'"
//@ diag: 24:17 note "candidate constructor not viable: no known conversion from 'const Y' to 'X &' for 1st argument"
//@ diag: 24:12 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 24:24 note "candidate function"
//@ diag: 32:8 error "no matching constructor for initialization of 'W<int>'"
//@ diag: 27:36 note "candidate constructor not viable: 1st argument ('const W<int>') would lose const qualifier"
//@ diag: 27:31 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 27:43 note "candidate function"
struct X { X(); X(X&); operator X() const; };
struct Y : X { Y(); };
const X cx;
template <class T> struct W { W(); W(W&); operator W() const; };
X y(cx);
const Y cy;
X z = cy;
const W<int> cw;
W<int> w(cw);
