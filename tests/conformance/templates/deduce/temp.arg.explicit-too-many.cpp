// [temp.arg.explicit]/2: too many explicit arguments make the candidate non-viable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'f'"
//@ diag: 6:25 note "candidate template ignored: substitution failure: too many template arguments for function"
template <class T> void f(T);
void g() { f<int, int>(1); }
