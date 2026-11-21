// spec/sema.md §4.5 (x86 `interrupt`), spec/modules.md §7 "Imported
// `used` functions" (WP-1825): an importer taking the address of an
// imported inline x86-64 interrupt handler (implicitly `used`) defines
// it `linkonce_odr` with the `x86_intrcc` convention and a `byval` frame
// parameter, in `llvm.compiler.used`, as clang emits it; before, kiln
// declared it as an ordinary function. One not named is not emitted.
//@ args: --target=x86_64-unknown-linux-gnu
//@ module: mod_x86_interrupt=Inputs/mod_x86_interrupt.cppm
import mod_x86_interrupt;
void *take() { return (void *)&isr; }
void *take_code() { return (void *)&isr_code; }
