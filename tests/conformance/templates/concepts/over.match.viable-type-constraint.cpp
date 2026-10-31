//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "no matching function for call to 'f'"
//@ diag: 9:23 note "candidate template ignored: constraints not satisfied [with T = char]"
//@ diag: 9:11 note "because 'char' does not satisfy 'Big'"
//@ diag: 8:34 note "because 'sizeof(char) > 4' (1 > 4) evaluated to false"
// [over.match.viable]/3: a candidate whose constraints are not satisfied.
template <class T> concept Big = sizeof(T) > 4;
template <Big T> void f(T);
void g() { f('c'); }
