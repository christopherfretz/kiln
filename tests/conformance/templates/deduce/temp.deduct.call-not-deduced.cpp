// [temp.deduct]/5: a parameter neither deduced nor defaulted fails (incomplete deduction).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'make'"
//@ diag: 6:31 note "candidate template ignored: couldn't infer template argument 'R'"
template <class R, class T> R make(T);
void g() { make(1); }
