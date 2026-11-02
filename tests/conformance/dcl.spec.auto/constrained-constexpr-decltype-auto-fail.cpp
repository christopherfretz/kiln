// [dcl.type.auto.deduct]/2.2, /4: the type-constraint of a `constexpr`
// `decltype(auto)` variable sees `decltype(e)` (`int`), not the variable's
// `const int`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:11 error "deduced type 'int' does not satisfy 'same<const int>'"
//@ diag: 8:44 note "because '__is_same(int, const int)' evaluated to false"
template <class T, class U> concept same = __is_same(T, U);
constexpr same<const int> decltype(auto) d = 1;
