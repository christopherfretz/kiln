// [over.match.viable]: every candidate removed by substitution failure.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:12 error "no matching function for call to 'f'"
//@ diag: 7:34 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used"
//@ diag: 8:34 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used"
template <class T> typename T::a f(T);
template <class T> typename T::b f(T);
void g() { f(1); }
