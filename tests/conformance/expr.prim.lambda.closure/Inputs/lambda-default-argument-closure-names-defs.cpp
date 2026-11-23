// The definitions for ../lambda-default-argument-closure-names-two-units-run.cpp,
// each under the name clang 19 and 22 give the specialization there. A
// closure type is an empty class, passed as one.
struct E {};
int a2u(E) __asm__("_Z3tk2IZZ2a2IiEivENKUliE_clEiEd_UlvE_EiT_");
int a3t() __asm__("_Z2tkIN2a3UlTyvE_UlvE_EEiv");
int rn(E) __asm__("_Z3tk2IN1r1L1nMUlvE_EEiT_");
int a2u(E) { return 1; }
int a3t() { return 2; }
int rn(E) { return 3; }
