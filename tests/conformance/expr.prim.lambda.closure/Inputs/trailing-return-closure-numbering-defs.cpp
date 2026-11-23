// The definition for ../trailing-return-closure-numbering-two-units-run.cpp,
// under the name clang 19 and 22 give the specialization there. A closure
// type is an empty class, passed as one.
struct E {};
int sw(E) __asm__("_Z3tk2IZ1svEUlvE0_EiT_");
int sw(E) { return 5; }
