// [temp.inst], [temp.deduct.general], [dcl.spec.auto.general]: a function
// template specialization whose instantiated body has an error is invalid
// (clang's InstantiateFunctionDefinition on a `StmtError` body): a later
// call that deduces it again fails with "substitution failure", and a
// placeholder return type is not deduced `void` from the dropped `return`
// (no follow-on "invalid operands ... 'void'"). An error in a variable's
// initializer leaves the declaration statement, and the specialization,
// valid (clang keeps the invalid variable; its uses are RecoveryExprs).
// WP-145.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:52 error "no member named 'nosuch' in 'S'"
//@ diag: 28:10 note "in instantiation of function template specialization 'h<S>' requested here"
//@ diag: 29:10 error "no matching function for call to 'h'"
//@ diag: 27:34 note "candidate template ignored: substitution failure [with T = S]"
//@ diag: 30:43 error "no member named 'nosuch' in 'S'"
//@ diag: 31:9 note "in instantiation of function template specialization 'm<S>' requested here"
//@ diag: 31:18 error "no matching function for call to 'm'"
//@ diag: 30:25 note "candidate template ignored: substitution failure [with T = S]"
//@ diag: 32:54 error "no member named 'nosuch' in 'S'"
//@ diag: 33:10 note "in instantiation of function template specialization 'h3<S>' requested here"
//@ diag: 34:10 error "no matching function for call to 'h3'"
//@ diag: 32:34 note "candidate template ignored: substitution failure [with T = S]"
//@ requires: clang>=22
// clang 22 spelling of invalid-body-substitution-failure.cpp (WP-394).
struct S {};
template <class T> constexpr int h(T t) { return t.nosuch(); }
int e1 = h(S{});
int e2 = h(S{});
template <class T> auto m(T t) { return t.nosuch(); }
int y = m(S{}) + m(S{});
template <class T> constexpr int h3(T t) { int x = t.nosuch(); x = 2; return 1; }
int e3 = h3(S{});
int e4 = h3(S{});
