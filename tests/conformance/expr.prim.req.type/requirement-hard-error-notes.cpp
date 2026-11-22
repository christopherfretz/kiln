// [expr.prim.req.general]/5, [temp.inst]: a hard error while
// substituting into a simple or type requirement (here a failed
// static_assert in a class template the requirement instantiates) is
// reported under "in instantiation of requirement here" at the
// requirement (a type requirement's `typename`), and the requirement's
// note is clang's unknown-substitution-error form, "would be invalid"
// without a message (clang `note_*_unknown_substitution_error`).
// WP-1860. Verified against clang 22 and 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:45 error "static assertion failed due to requirement 'sizeof(int) > 4'"
//@ diag: 33:44 note "in instantiation of template class 'B<int>' requested here"
//@ diag: 33:44 note "in instantiation of requirement here"
//@ diag: 33:33 note "while substituting template arguments into constraint expression here"
//@ diag: 34:15 note "while checking the satisfaction of concept 'C1<int>' requested here"
//@ diag: 32:55 note "expression evaluates to '4 > 4'"
//@ diag: 34:15 error "static assertion failed"
//@ diag: 34:15 note "because 'int' does not satisfy 'C1'"
//@ diag: 33:44 note "because 'B<X>::f()' would be invalid"
//@ diag: 32:45 error "static assertion failed due to requirement 'sizeof(bool) > 4'"
//@ diag: 35:53 note "in instantiation of template class 'B<bool>' requested here"
//@ diag: 35:44 note "in instantiation of requirement here"
//@ diag: 35:33 note "while substituting template arguments into constraint expression here"
//@ diag: 36:15 note "while checking the satisfaction of concept 'C2<bool>' requested here"
//@ diag: 32:55 note "expression evaluates to '1 > 4'"
//@ diag: 36:15 error "static assertion failed"
//@ diag: 36:15 note "because 'bool' does not satisfy 'C2'"
//@ diag: 35:44 note "because 'typename B<X>::t' would be invalid"
//@ diag: 38:15 error "static assertion failed"
//@ diag: 38:15 note "because 'int' does not satisfy 'C3'"
//@ diag: 37:53 note "because 'typename X::nope' would be invalid: type 'int' cannot be used prior to '::' because it has no members"
template <class T> struct B { static_assert(sizeof(T) > 4); using t = int; static int f(); };
template <class X> concept C1 = requires { B<X>::f(); };
static_assert(C1<int>);
template <class X> concept C2 = requires { typename B<X>::t; };
static_assert(C2<bool>);
template <class X> concept C3 = requires { typename X::nope; };
static_assert(C3<int>);
