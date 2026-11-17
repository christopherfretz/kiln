// C: the function types of `void (int)` and its `noreturn` variant are
// compatible for clang beyond simple assignment: a pointer to either
// pointer converts, a conditional and a comparison take them, and
// `__builtin_types_compatible_p` answers 1 (GCC 16.1 is stricter, with
// -Wincompatible-pointer-types warnings). clang 19 still rejected the
// pointer-to-pointer assignment (-Wincompatible-function-pointer-types).
// WP-1551.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
typedef void (*nfp)(int) __attribute__((noreturn));
typedef void (*fp)(int);
nfp *x;
fp *y;
void t(void) { x = y; y = x; }
int c(int k, nfp a, fp b) { return (k ? a : b) == 0 && a != b; }
_Static_assert(__builtin_types_compatible_p(nfp, fp), "");
