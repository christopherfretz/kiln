// [expr.call]/7 (a parameter is copy-initialized from its argument), with
// AAPCS64 §6.8.2 C.10 (spec/abi.md §8.12, spec/landed/abi/WP-583.md): a
// 16-byte composite whose alignment comes only from `alignas` is classified
// by its unadjusted (natural) alignment, 8, so after an `int` it takes x1:x2
// rather than the even pair x2:x3. Kiln calling kiln agrees with itself
// whatever it does, so the other side of each call is written in assembly
// to the AAPCS64 rule: `asm_lo`/`asm_hi`/`asm_only_hi` are callees, and
// `asm_call_dump`/`asm_call_vdump` call kiln's `dump` and variadic `vdump`
// with the fields in x1 and x2.
//@ expect: run
//@ requires: target-aarch64
//@ strict-diags
//@ stdout: 7 1 1 | U 3 7 1 | V 1 7 1 | U 3 7 1 | V 1 7 1
#include <stdarg.h>
extern "C" int printf(const char *, ...);

struct alignas(16) U {
  unsigned long lo, hi;
};
static_assert(sizeof(U) == 16 && alignof(U) == 16);

extern "C" unsigned long asm_lo(int, U);
extern "C" unsigned long asm_hi(int, U);
extern "C" unsigned long asm_only_hi(U);
extern "C" void asm_call_dump();
extern "C" void asm_call_vdump();

extern "C" __attribute__((noinline, used)) void dump(int x, U u) {
  printf(" | U %d %lu %lu", x, u.lo, u.hi);
}
extern "C" __attribute__((noinline, used)) void vdump(int n, ...) {
  va_list ap;
  va_start(ap, n);
  U u = va_arg(ap, U);
  va_end(ap);
  printf(" | V %d %lu %lu", n, u.lo, u.hi);
}

asm(R"(
  .text
  .p2align 2
  .globl asm_lo
asm_lo:
  mov x0, x1
  ret
  .globl asm_hi
asm_hi:
  mov x0, x2
  ret
  .globl asm_only_hi
asm_only_hi:
  mov x0, x1
  ret
  .globl asm_call_dump
asm_call_dump:
  stp x29, x30, [sp, #-16]!
  mov x29, sp
  mov w0, #3
  mov x1, #7
  mov x2, #1
  bl dump
  ldp x29, x30, [sp], #16
  ret
  .globl asm_call_vdump
asm_call_vdump:
  stp x29, x30, [sp, #-16]!
  mov x29, sp
  mov w0, #1
  mov x1, #7
  mov x2, #1
  bl vdump
  ldp x29, x30, [sp], #16
  ret
)");

int main() {
  U u{7, 1};
  printf("%lu %lu %lu", asm_lo(3, u), asm_hi(3, u), asm_only_hi(u));
  asm_call_dump();
  asm_call_vdump();
  dump(3, u);
  vdump(1, u);
  printf("\n");
}
