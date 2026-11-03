// [temp.deduct.type]/10: `T&&` against `int&` deduces `T = int&`, against
// `int&&` deduces `T = int`; both in one parameter-type-list conflict.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:26 error "address of overloaded function 'm' does not match required type 'int (int &, int &&)'"
//@ diag: 7:24 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int &' vs. 'int')"
template <class T> int m(T&&, T&&);
int (*p4)(int&, int&&) = m;
