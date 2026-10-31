// [temp.deduct.type]: a P that cannot match A is noted with both types.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'f'"
//@ diag: 6:25 note "candidate template ignored: could not match 'T *' against 'int'"
template <class T> void f(T*);
void g() { f(1); }
