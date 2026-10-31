//@ expect: compile-ok
// [dcl.fct]/22, [temp.func.order]/6: abbreviated templates are ordered by
// their invented parameters' constraints.
template <class T> concept A = sizeof(T) >= 1;
template <class T> concept B = A<T> && (sizeof(T) >= 4);
constexpr int f(A auto) { return 1; }
constexpr int f(B auto) { return 2; }
static_assert(f('a') == 1);
static_assert(f(1) == 2);
