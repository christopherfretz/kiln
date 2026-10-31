// [over.over]/1: an overload set converted to a non-function type
// cannot be resolved; every candidate is noted, templates last when
// their deduction failed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:9 error "address of overloaded function 'g' does not match required type 'int'"
//@ diag: 20:6 note "candidate function"
//@ diag: 19:6 note "candidate function"
//@ diag: 18:6 note "candidate function"
//@ diag: 25:9 error "address of overloaded function 'm' does not match required type 'int'"
//@ diag: 24:6 note "candidate function"
//@ diag: 22:6 note "candidate function"
//@ diag: 23:25 note "candidate template ignored: could not match 'void (T, T)' against 'int'"
//@ diag: 29:9 error "address of overloaded function 'n' does not match required type 'int'"
//@ diag: 28:6 note "candidate function"
//@ diag: 27:6 note "candidate function"
//@ diag: 26:25 note "candidate template ignored: could not match 'void (T, T)' against 'int'"
void g(int);
void g(long);
void g(char);
int x = g;
void m(int);
template <class T> void m(T, T);
void m(char);
int y = m;
template <class T> void n(T, T);
void n(int);
void n(char);
int z = n;
