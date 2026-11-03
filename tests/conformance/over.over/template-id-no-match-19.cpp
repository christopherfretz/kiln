// [over.over]/1, /3: a function template-id that is not called stays an
// overload set until its target is known. An initialization (or
// `return`) of a pointer or reference to function whose type the one
// specialization named does not have finds no match, and that
// specialization is not used: the deprecated volatile return type
// warnings its substitution held back ([temp.deduct.general]/7,
// [depr.volatile.type]/2) are not reported there. A use that converts
// reports them, also after one that found no match.
//@ std: c++26
//@ requires: clang<22
//@ strict-diags
//@ expect: compile-fail
//@ diag: 31:15 error "address of overloaded function 'f' does not match required type 'int ()'"
//@ diag: 30:21 note "candidate template ignored: could not match 'volatile int ()' against 'int ()'"
//@ diag: 32:14 error "address of overloaded function 'f' does not match required type 'int ()'"
//@ diag: 30:21 note "candidate template ignored: could not match 'volatile int ()' against 'int ()'"
//@ diag: 33:17 error "address of overloaded function 'f' does not match required type 'int ()'"
//@ diag: 30:21 note "candidate template ignored: could not match 'volatile int ()' against 'int ()'"
//@ diag: 34:13 error "address of overloaded function 'f' does not match required type 'int ()'"
//@ diag: 30:21 note "candidate template ignored: could not match 'volatile int ()' against 'int ()'"
//@ diag: 35:25 error "address of overloaded function 'f' does not match required type 'int ()'"
//@ diag: 30:21 note "candidate template ignored: could not match 'volatile int ()' against 'int ()'"
//@ diag: 37:15 error "address of overloaded function 'g' does not match required type 'int ()'"
//@ diag: 36:21 note "candidate template ignored: could not match 'volatile long ()' against 'int ()'"
//@ diag: 38:15 error "address of overloaded function 'g' does not match required type 'void ()'"
//@ diag: 36:21 note "candidate template ignored: could not match 'volatile int ()' against 'void ()'"
//@ diag: 39:19 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 30:21 warning "volatile-qualified return type 'volatile int' is deprecated"
//@ diag: 30:19 warning "volatile-qualified return type 'volatile int' is deprecated"
template<class T> T f() { return T(); }
int (*p)() = &f<volatile int>;
int (*q)() = f<volatile int>;
int (*r)() = (&(f<volatile int>));
int (*d)()(&f<volatile int>);
int (*ret())() { return f<volatile int>; }
template<class T> T g() { return T(); }
int (&rf)() = g<volatile long>;
void (*h)() = g<volatile int>;
volatile int (*ok)() = &f<volatile int>;
