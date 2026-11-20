// In C the AAPCS64 `__builtin_va_list` is `struct __va_list` at file scope
// (`%struct.__va_list`); only C++ puts it in namespace `std` (clang
// `CreateAArch64ABIBuiltinVaListDecl`). Found while comparing the AAPCS64
// pure scalable type lowering with clang (WP-1739).
//@ requires: target-aarch64
//@ expect: ir
//@ xfail(kiln): WP-1739 follow-up, sema declares the AArch64 va_list record in an implicit namespace std in C too, so its IR name is %"struct.std::__va_list"
int first(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  int r = __builtin_va_arg(ap, int);
  __builtin_va_end(ap);
  return r;
}
