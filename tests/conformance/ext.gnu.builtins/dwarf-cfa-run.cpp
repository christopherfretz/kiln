// GNU unwinder builtins at run time on the host (found in unwinder-style
// code): `__builtin_dwarf_cfa()` is the canonical frame address, the
// caller's stack pointer at the call, so it lies above the callee's
// locals and at or below the caller's frame address; `__builtin_unwind_init`
// leaves the program running; `__builtin_eh_return_data_regno(0)` and
// `(1)` are 0 and 1 on x86-64 and AArch64 (the DWARF numbers of `%rax` /
// `%rdx` and `x0` / `x1`), any other N -1. WP-1428.
//@ expect: run
//@ stdout: 1 1 0 1 -1
#include <cstdio>
__attribute__((noinline)) void *cfa(void **local) {
  int x = 0;
  *local = &x;
  __builtin_unwind_init();
  return __builtin_dwarf_cfa();
}
int main() {
  void *caller_frame = __builtin_frame_address(0);
  void *local = nullptr;
  void *c = cfa(&local);
  std::printf("%d %d %d %d %d\n", (char *)c > (char *)local,
              (char *)c <= (char *)caller_frame, __builtin_eh_return_data_regno(0),
              __builtin_eh_return_data_regno(1), __builtin_eh_return_data_regno(2));
}
