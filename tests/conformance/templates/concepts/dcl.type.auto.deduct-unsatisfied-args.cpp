//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:1 error "deduced type 'int' does not satisfy 'Same<long>'"
//@ diag: 6:44 note "because '__is_same(int, long)' evaluated to false"
// [dcl.type.auto.deduct]/2: a placeholder type-constraint with arguments.
template <class T, class U> concept Same = __is_same(T, U);
Same<long> auto y = 2;
