//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:1 error "deduced type 'long' does not satisfy 'Int'"
//@ diag: 6:34 note "because '__is_same(long, int)' evaluated to false"
// [dcl.type.auto.deduct]/2: the placeholder under a pointer declarator.
template <class T> concept Int = __is_same(T, int);
long l;
Int auto *p = &l;
