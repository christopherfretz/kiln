//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "no matching function for call to 'p'"
//@ diag: 9:41 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 9:29 note "because 'int' does not satisfy 'Ptr'"
//@ diag: 8:34 note "because '__is_pointer(int)' evaluated to false"
// [over.match.viable]/3: a requires-clause naming a concept.
template <class T> concept Ptr = __is_pointer(T);
template <class T> requires Ptr<T> void p(T);
void q() { p(1); }
