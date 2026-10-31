// [temp.deduct]/8: the substitution failure is noted with the deduced arguments.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'f'"
//@ diag: 6:37 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used"
template <class T> typename T::type f(T);
void g() { f(1); }
