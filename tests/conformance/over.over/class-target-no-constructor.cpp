// [over.over]/1: without a constructor taking the overload set, the set
// is resolved against the class type and finds no match.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:14 error "address of overloaded function 'f' does not match required type 'S'"
//@ diag: 14:6 note "candidate function"
//@ diag: 13:6 note "candidate function"
//@ diag: 16:19 error "address of overloaded function 'f' does not match required type 'S'"
//@ diag: 14:6 note "candidate function"
//@ diag: 13:6 note "candidate function"
//@ diag: 16:17 note "passing argument to parameter here"
struct S {};
void f();
void f(int);
const S& s = f;
void v(const S& = f);
