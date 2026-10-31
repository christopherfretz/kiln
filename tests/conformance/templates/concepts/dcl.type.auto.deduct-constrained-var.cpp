//@ expect: compile-ok
// [dcl.type.auto.deduct]/2: the deduced type satisfies the placeholder's
// type-constraint.
template <class T> concept Int = __is_same(T, int);
template <class T, class U> concept Same = __is_same(T, U);
Int auto x = 1;
const Int auto &r = x;
Same<long> auto y = 2L;
Int auto *p = &x;
decltype(auto) d = x;
