//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3.5 -4.5 5 6 7 -8 9 10
//@ stdout: 2 4 6 8 10 12 14 16 18 20 22 24
// WP-237: va_arg of _Complex T ([cstdarg.syn]/1, C23 7.16.1.1) through
// the target's va_list walk (spec/abi.md §8.17): complex floating values
// from the SIMD/SSE registers and the stack, complex integers from the
// general registers, enough of each to spill past the register save area.
extern "C" int printf(const char *, ...);
using CF = _Complex float;
using CD = _Complex double;
using CL = _Complex long double;
using CI = _Complex int;
using CLL = _Complex long long;

void mixed(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  CF f = __builtin_va_arg(ap, CF);
  CD d = __builtin_va_arg(ap, CD);
  CL l = __builtin_va_arg(ap, CL);
  CI i = __builtin_va_arg(ap, CI);
  CLL ll = __builtin_va_arg(ap, CLL);
  __builtin_va_end(ap);
  printf("%g %g %g %g %Lg %Lg %d %d %lld %lld\n", __real__ f, __imag__ f,
         __real__ d, __imag__ d, __real__ l, __imag__ l, __real__ i, __imag__ i,
         __real__ ll, __imag__ ll);
}

void many(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  for (int k = 0; k < n; ++k) {
    CD d = __builtin_va_arg(ap, CD);
    printf(k ? " %g %g" : "%g %g", __real__ d * 2, __imag__ d * 2);
  }
  __builtin_va_end(ap);
  printf("\n");
}

int main() {
  mixed(0, CF{1, 2}, CD{3.5, -4.5}, CL{5, 6}, CI{7, -8}, CLL{9, 10});
  many(6, CD{1, 2}, CD{3, 4}, CD{5, 6}, CD{7, 8}, CD{9, 10}, CD{11, 12});
}
