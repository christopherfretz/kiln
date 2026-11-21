// spec/sema.md §4.5 (x86 `interrupt`), WP-1745, WP-1833: a handler whose
// frame pointee is a class never defined builds and runs. clang gives its
// `byval` parameter an opaque struct type, which LLVM's verifier rejects
// ("Attribute 'byval' does not support unsized types!"; clang's driver
// does not verify its own IR); kiln uses `byval(i8)`, as for `void *`
// (spec/codegen.md "Deviations").
//@ expect: run
//@ requires: target-x86_64
//@ stdout: 1
extern "C" int printf(const char *, ...);
struct frame;
__attribute__((interrupt)) void isr(frame *) {}

int main() { printf("%d\n", (void *)&isr != nullptr); }
