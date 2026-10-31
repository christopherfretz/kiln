//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:12 error "no matching function for call to 'f'"
//@ diag: 9:29 note "candidate template ignored: constraints not satisfied [with T = char]"
//@ diag: 9:11 note "because 'Same<char, int>' evaluated to false"
//@ diag: 8:44 note "because '__is_same(char, int)' evaluated to false"
// [temp.param]/4: `C<U> T` checks C<T, U>.
template <class T, class U> concept Same = __is_same(T, U);
template <Same<int> T> void f(T);
void g() { f('c'); }
