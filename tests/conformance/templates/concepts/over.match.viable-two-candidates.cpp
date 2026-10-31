//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:12 error "no matching function for call to 'f'"
//@ diag: 13:25 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 13:11 note "because 'int' does not satisfy 'Small'"
//@ diag: 11:36 note "because 'sizeof(int) == 1' (4 == 1) evaluated to false"
//@ diag: 14:24 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 14:11 note "because 'int' does not satisfy 'Huge'"
//@ diag: 12:35 note "because 'sizeof(int) > 8' (4 > 8) evaluated to false"
// [over.match.viable]/3: every unsatisfied candidate is explained.
template <class T> concept Small = sizeof(T) == 1;
template <class T> concept Huge = sizeof(T) > 8;
template <Small T> void f(T);
template <Huge T> void f(T);
void g() { f(1); }
