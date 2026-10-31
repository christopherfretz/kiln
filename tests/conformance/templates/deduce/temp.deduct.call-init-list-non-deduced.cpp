// [temp.deduct.call]/1: a braced list against plain T is a non-deduced context.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:12 error "no matching function for call to 'f'"
//@ diag: 6:25 note "candidate template ignored: couldn't infer template argument 'T'"
template <class T> void f(T);
void g() { f({1, 2}); }
