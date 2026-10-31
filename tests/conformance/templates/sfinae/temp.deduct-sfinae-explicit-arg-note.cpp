// [temp.arg.explicit]: a substitution failure with explicit arguments is noted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'f'"
//@ diag: 6:25 note "candidate template ignored: substitution failure [with T = int]: type 'int' cannot be used"
template <class T> void f(typename T::type);
void g() { f<int>(0); }
