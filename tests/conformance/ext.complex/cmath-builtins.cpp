//@ expect: run
//@ strict-diags
//@ stdout: cabs 5 carg 0.927
//@ stdout: cexp 1 0 csqrt 0 1
//@ stdout: cpow -1 0 cproj 1 2
//@ stdout: f 5 l 5
// The C99 <complex.h> builtins `__builtin_c*` (WP-237; libstdc++ <complex>
// calls them): declared with clang's `Builtins.td` types (`X` = `_Complex`),
// lowered to calls of the library function through the complex call ABI.
// C23 7.3.8-7.3.9 (via [complex.numbers]/[cmplx.over]); spec/abi.md §8.17.
extern "C" int printf(const char *, ...);
using C = _Complex double;
int main() {
  C z{3, 4};
  printf("cabs %g carg %.3f\n", __builtin_cabs(z), __builtin_carg(z));
  C e = __builtin_cexp(C{0, 0}), s = __builtin_csqrt(C{-1, 0});
  printf("cexp %g %g csqrt %g %g\n", __real__ e, __imag__ e, __real__ s, __imag__ s);
  C p = __builtin_cpow(C{0, 1}, C{2, 0}), j = __builtin_cproj(C{1, 2});
  printf("cpow %.0f %.0f cproj %g %g\n", __real__ p, __imag__ p + 0.0, __real__ j, __imag__ j);
  _Complex float zf{3, 4};
  _Complex long double zl{3, 4};
  printf("f %g l %Lg\n", (double)__builtin_cabsf(zf), __builtin_cabsl(zl));
}
