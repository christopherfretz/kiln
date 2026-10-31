//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:12 error "no matching function for call to 'h'"
//@ diag: 7:25 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 7:40 note "because 'sizeof(int) == 1' (4 == 1) evaluated to false"
// [over.match.viable]/3: a trailing requires-clause that is not satisfied.
template <class T> void h(T) requires (sizeof(T) == 1);
void k() { h(1); }
