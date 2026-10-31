//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:12 error "no matching function for call to 'f'"
//@ diag: 8:71 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 8:30 note "because 'sizeof(int) == 1' (4 == 1) evaluated to false"
//@ diag: 8:50 note "and 'sizeof(int) == 8' (4 == 8) evaluated to false"
// [over.match.viable]/3: an unsatisfied disjunction in a requires-clause.
template <class T> requires (sizeof(T) == 1) || (sizeof(T) == 8) void f(T);
void g() { f(1); }
