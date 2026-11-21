// [dcl.inline]/6, spec/sema.md §4.5 (x86 `interrupt`), spec/modules.md §7
// "Imported `used` functions" (WP-1825): an importer that takes the
// address of an imported inline x86-64 interrupt handler defines it
// `linkonce_odr` with the `x86_intrcc` convention and a `byval` frame,
// in `llvm.compiler.used`, as clang 22 does (before, kiln only declared
// it, without the convention); it is the module's function. Under
// `--bmi-image` from the declaration image
// (`tests/codegen/module_x86_interrupt_importer.cpp` pins the IR).
//@ expect: run
//@ std: c++20
//@ requires: target-x86_64
//@ module: imgxi=Inputs/img-x86-interrupt.cppm
//@ stdout: 1 1 1
import imgxi;
extern "C" int printf(const char *, ...);

int main() {
  void *a = (void *)&isr;
  void *b = (void *)&isr_code;
  printf("%d %d %d\n", a == module_isr(), a != nullptr, b != a);
}
