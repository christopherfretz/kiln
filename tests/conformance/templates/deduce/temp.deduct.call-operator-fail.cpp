// [over.match.oper]: an operator template whose deduction fails leaves no viable candidate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:22 error "invalid operands to binary expression ('V<int>' and 'V<long>')"
//@ diag: 7:25 note "candidate template ignored: deduced conflicting types for parameter 'T' ('int' vs. 'long')"
template <class T> struct V { T x; };
template <class T> V<T> operator+(V<T> a, V<T> b);
void g() { V<int>{1} + V<long>{2}; }
