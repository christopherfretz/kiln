//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:25 error "redefinition of 'f3'"
//@ diag: 9:25 note "previous definition is here"
//@ diag: 12:25 error "redefinition of 'f4'"
//@ diag: 11:25 note "previous definition is here"
// Requires-clauses with sizeof/type-trait operands naming template parameters
// are equivalent by parameter position ([temp.over.link]/5; WP-109).
template <class T> void f3(T) requires (sizeof(T) > 1) {}
template <class T> void f3(T) requires (sizeof(T) > 1) {}
template <class U> void f4(U) requires (__is_same(U, int)) {}
template <class T> void f4(T) requires (__is_same(T, int)) {}
template <class T> void f5(T) requires (alignof(T) > 1) {}
template <class T> void f5(T) requires (sizeof(T) > 1) {}
