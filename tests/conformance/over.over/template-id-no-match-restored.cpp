// [over.over]/1, /3, [temp.deduct.general]/7: the deprecated volatile
// return type warnings a function template specialization's substitution
// held back are not reported at a use that finds no match; the first use
// that converts reports them. clang 22 stores a deduction's warnings
// only when the deduction succeeds, so their context notes name the
// deduction of that later use, not of the one that found no match
// (clang 19 has no such notes: template-id-no-match-19.cpp).
//@ std: c++26
//@ requires: clang>=22
//@ strict-diags
//@ expect: compile-fail
//@ diag: 23:15 error "address of overloaded function 'f' does not match required type 'int ()'"
//@ diag: 22:21 note "candidate template ignored: could not match 'volatile int ()' against 'int ()'"
//@ diag: 24:16 error "address of overloaded function 'f' does not match required type 'int ()'"
//@ diag: 22:21 note "candidate template ignored: could not match 'volatile int ()' against 'int ()'"
//@ diag: 25:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 22:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 25:25 note "while substituting explicitly-specified template arguments into function template 'f' "
//@ diag: 22:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 25:25 note "while substituting deduced template arguments into function template 'f' [with T = volatile int]"
//@ diag: 26:22 warning "volatile-qualified return type 'volatile int' is deprecated"
template<class T> T f() { return T(); }
int (*p)() = &f<volatile int>;
int (*q)() = &(f<volatile int>);
volatile int (*ok)() = &f<volatile int>;
volatile int (*again)() = &f<volatile int>;
