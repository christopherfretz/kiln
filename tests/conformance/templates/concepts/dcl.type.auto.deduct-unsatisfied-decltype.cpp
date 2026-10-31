//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:1 error "deduced type 'int &' does not satisfy 'Int'"
//@ diag: 6:34 note "because '__is_same(int &, int)' evaluated to false"
// [dcl.type.auto.deduct]/2: `C decltype(auto)` checks decltype(E).
template <class T> concept Int = __is_same(T, int);
int i;
Int decltype(auto) r = (i);
