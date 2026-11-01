// The C99 <complex.h> builtins (WP-237): calls of the library function
// with clang's complex call ABI; `__builtin_cprojl` is `const`, and an
// indirect x86_64 `long double _Complex` adds `argmem: readwrite`
// (clang `AddPotentialArgAccess`). spec/abi.md §8.17.
_Complex double z(_Complex double a, _Complex double b) { return __builtin_cpow(__builtin_cexp(a), b); }
_Complex float zf(_Complex float a) { return __builtin_csqrtf(a); }
_Complex long double zl(_Complex long double a) { return __builtin_cprojl(a); }
double ab(_Complex double a) { return __builtin_cabs(a) + __builtin_carg(a); }
float abf(_Complex float a) { return __builtin_cabsf(a); }
long double abl(_Complex long double a) { return __builtin_cargl(a); }
