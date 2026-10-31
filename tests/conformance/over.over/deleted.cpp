// [over.over], [dcl.fct.def.delete]/2: taking the address of the
// selected deleted function (or deleted template specialization) is an
// error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:18 error "attempt to use a deleted function"
//@ diag: 16:6 note "'k' has been explicitly marked deleted here"
//@ diag: 19:18 error "attempt to use a deleted function"
//@ diag: 16:6 note "'k' has been explicitly marked deleted here"
//@ diag: 21:18 error "attempt to use a deleted function"
//@ diag: 20:25 note "'j<int>' has been explicitly marked deleted here"
//@ diag: 22:18 error "attempt to use a deleted function"
//@ diag: 20:25 note "'j<int>' has been explicitly marked deleted here"
//@ diag: 24:18 error "attempt to use a deleted function"
//@ diag: 23:25 note "'kk<int>' has been explicitly marked deleted here"
void k(int) = delete;
void k(long);
void (*u)(int) = &k;
void (*v)(int) = k;
template <class T> void j(T) = delete;
void (*w)(int) = j;
void (*x)(int) = (&j);
template <class T> void kk(T) = delete;
void (*y)(int) = &kk;
