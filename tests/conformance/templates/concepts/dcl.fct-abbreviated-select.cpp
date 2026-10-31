//@ expect: compile-ok
// [dcl.fct]/22: `C auto` parameters are constrained invented template
// parameters.
template <class T> concept Small = sizeof(T) == 1;
template <class T> concept Big = sizeof(T) > 1;
constexpr int f(Small auto) { return 1; }
constexpr int f(Big auto) { return 2; }
static_assert(f('a') == 1);
static_assert(f(1) == 2);
