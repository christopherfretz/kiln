// [temp.arg.explicit]/3: an explicit argument of the wrong kind fails deduction.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'f'"
//@ diag: 6:22 note "candidate template ignored: invalid explicitly-specified argument for template parameter '"
template <int N> int f() { return N; }
void g() { f<int>(); }
