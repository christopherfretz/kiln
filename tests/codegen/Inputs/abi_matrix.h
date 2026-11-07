// The differential calling-convention matrix (spec/landed/abi/WP-766.md):
// `ABI_SHAPE(T)` emits, for one by-value type T, every call-boundary
// position whose lowering clang's ABIInfo decides — a declaration (the
// caller's view), a definition (the callee's view), a call site, a
// parameter after the argument registers are exhausted (both register
// files, then only the integer one), a variadic argument and a `va_arg`
// read. The tests/codegen/abi_matrix_*.cpp cases instantiate it per
// shape; `ir_vs_clang` compares the IR with clang 19 and 22 on both
// AArch64 and x86-64 ([expr.call]/7, [class.temporary]/3, Itanium C++
// ABI §3.1.2, AAPCS64 §6.8, SysV x86-64 psABI §3.2.3).
#ifndef KILN_ABI_MATRIX_H
#define KILN_ABI_MATRIX_H

extern "C" void abi_sink(int, ...);

#define ABI_GPRS long, long, long, long, long, long, long, long
#define ABI_FPRS double, double, double, double, double, double, double, double

// The positions every class type can take, variadic ones excluded
// (passing a non-trivially-copyable class through `...` is ill-formed in
// clang, [expr.call]/12 conditionally-supported).
#define ABI_SHAPE_FIXED(T)                                                    \
  T T##_ext(T);                                                               \
  extern "C" T T##_c(T, int);                                                 \
  T T##_id(T a) { return a; }                                                 \
  T T##_stack(ABI_GPRS, ABI_FPRS, T a) { return a; }                          \
  T T##_tail(long, long, long, long, long, long, long, T a) { return a; }     \
  void T##_fcall(T *p) {                                                      \
    T r = T##_ext(*p);                                                        \
    T##_c(r, 1);                                                              \
  }

// A trivially copyable class that is not standard-layout can be passed
// through `...` but not read by `va_arg` (-Wnon-pod-varargs, an error).
#define ABI_SHAPE_NP(T)                                                       \
  ABI_SHAPE_FIXED(T)                                                          \
  void T##_vcall(T *p) { abi_sink(1, *p, T##_ext(*p)); }

#define ABI_SHAPE(T)                                                          \
  ABI_SHAPE_FIXED(T)                                                          \
  T T##_va(int n, ...) {                                                      \
    __builtin_va_list ap;                                                     \
    __builtin_va_start(ap, n);                                                \
    T r = __builtin_va_arg(ap, T);                                            \
    __builtin_va_end(ap);                                                     \
    return r;                                                                 \
  }                                                                           \
  void T##_vcall(T *p) { abi_sink(1, *p, T##_ext(*p)); }

#endif
