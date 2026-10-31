//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:15 error "static assertion failed"
//@ diag: 8:15 note "because 'Same<int, char>' evaluated to false"
//@ diag: 7:44 note "because '__is_same(int, char)' evaluated to false"
// [temp.constr.atomic]: a multi-argument concept's false atom.
template <class T, class U> concept Same = __is_same(T, U);
static_assert(Same<int, char>);
