// [expr.prim.id.qual]/1, [temp.inst]/2: a class named before '::' must be
// complete, which instantiates it; an undefined template cannot be.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:9 error "implicit instantiation of undefined template 'Fwd<int>'"
//@ diag: 7:27 note "template is declared here"
template <class T> struct Fwd;
int v = Fwd<int>::x;
