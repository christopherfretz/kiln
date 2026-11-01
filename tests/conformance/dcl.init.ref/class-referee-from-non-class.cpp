// [dcl.init.ref]/5.4.1: binding a reference to class type to a non-class initializer creates a
// temporary by user-defined conversion; with no usable constructor the reference 'could not bind',
// an ambiguous one is 'conversion ... is ambiguous', a deleted one is 'attempt to use a deleted function'
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:10 error "reference to type 'const B' could not bind to an rvalue of type 'int'"
//@ diag: 22:10 error "reference to type 'const D' could not bind to an rvalue of type 'int'"
//@ diag: 23:10 error "reference to type 'const E' could not bind to an rvalue of type 'int'"
//@ diag: 24:5 error "reference to type 'B' could not bind to an rvalue of type 'int'"
//@ diag: 25:14 error "conversion from 'int' to 'const F' is ambiguous"
//@ diag: 19:12 note "candidate constructor"
//@ diag: 19:21 note "candidate constructor"
//@ diag: 26:14 error "attempt to use a deleted function"
//@ diag: 20:12 note "'G' has been explicitly marked deleted here"
//@ diag: 27:10 error "reference to type 'const B' could not bind to an rvalue of type 'int'"
struct B { explicit B(int); };
struct D { D(char*); };
struct E { };
struct F { F(long); F(short); };
struct G { G(int) = delete; };
const B& b = 1;
const D& d = 1;
const E& e = 1;
B&& b2 = 1;
const F& f = 1;
const G& g = 1;
const B& b3(1);
