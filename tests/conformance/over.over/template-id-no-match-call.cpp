// [over.over]/1, /3, [temp.deduct.general]/7: a call after a use of a
// function template-id that found no match reports only the warnings of
// the specialization's signature, not those of the failed use's
// explicit-argument substitution (a call substitutes explicit arguments
// into the parameter types alone). clang 22 notes the call's deduction.
//@ std: c++26
//@ requires: clang>=22
//@ strict-diags
//@ expect: compile-fail
//@ diag: 15:15 error "address of overloaded function 'f' does not match required type 'int ()'"
//@ diag: 14:21 note "candidate template ignored: could not match 'volatile int ()' against 'int ()'"
//@ diag: 14:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 16:12 note "while substituting deduced template arguments into function template 'f' [with T = volatile int]"
template<class T> T f() { return T(); }
int (*p)() = &f<volatile int>;
void g() { f<volatile int>(); }
