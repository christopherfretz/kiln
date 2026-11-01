// `va_arg` of `_Complex T` (clang `ComplexExprEmitter::VisitVAArgExpr`
// through the target `EmitVAArg`): AAPCS64 reads a complex floating
// value as a two-element HFA and a complex integer from the general
// registers; SysV x86-64 reads `_Complex float` as one SSE eightbyte,
// `_Complex double` as two, `_Complex long double` from memory and
// complex integers as INTEGER; [cstdarg.syn], spec/abi.md §8.17.
using CF = _Complex float;
using CD = _Complex double;
using CL = _Complex long double;
using CI = _Complex int;
using CLL = _Complex long long;
using CC = _Complex char;
CF pf(__builtin_va_list ap) { return __builtin_va_arg(ap, CF); }
CD pd(__builtin_va_list ap) { return __builtin_va_arg(ap, CD); }
CL pl(__builtin_va_list ap) { return __builtin_va_arg(ap, CL); }
CI pi(__builtin_va_list ap) { return __builtin_va_arg(ap, CI); }
CLL pll(__builtin_va_list ap) { return __builtin_va_arg(ap, CLL); }
CC pc(__builtin_va_list ap) { return __builtin_va_arg(ap, CC); }
double re(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  CD z = __builtin_va_arg(ap, CD);
  double r = __real__ __builtin_va_arg(ap, CF) + __imag__ z;
  __builtin_va_arg(ap, CI);
  __builtin_va_end(ap);
  return r;
}
