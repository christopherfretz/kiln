// GCC's __builtin_va_arg_pack() and __builtin_va_arg_pack_len() forward
// an always-inlined variadic function's anonymous arguments. clang, and
// kiln with it, does not implement them (builtin-va-arg-pack-run.c is the
// xfail repro): glibc's fortify wrappers use them only when
// __GNUC_PREREQ (4,3), which kiln's __GNUC__ 4 / __GNUC_MINOR__ 2 do not
// meet, so they take the clang path. In C an undeclared `__builtin_` name
// is clang's "use of unknown builtin" (an error by default). WP-1550.
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:7 error "use of unknown builtin '__builtin_va_arg_pack_len'"
//@ diag: 16:20 error "use of unknown builtin '__builtin_va_arg_pack'"
int printf(const char *, ...);
extern inline __attribute__((always_inline, gnu_inline)) int myprintf(const char *f, ...) {
  if (__builtin_va_arg_pack_len() > 2) return -1;
  return printf(f, __builtin_va_arg_pack());
}
int main(void) { return myprintf("%d\n", 1); }
