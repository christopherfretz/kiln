// spec/sema.md §4.5 (x86 `interrupt`), WP-1745: a handler whose frame
// pointee is a class never defined gets a `byval` parameter of an opaque
// type, as in clang's IR, which LLVM's verifier rejects ("Attribute
// 'byval' does not support unsized types!"). clang's driver does not
// verify its own IR, so the program builds; kiln's backend step verifies
// it and fails.
//@ expect: run
//@ requires: target-x86_64
//@ xfail(kiln): WP-1833, an interrupt handler's byval frame of an incomplete class fails LLVM's verifier at -c
//@ stdout: 1
extern "C" int printf(const char *, ...);
struct frame;
__attribute__((interrupt)) void isr(frame *) {}

int main() { printf("%d\n", (void *)&isr != nullptr); }
