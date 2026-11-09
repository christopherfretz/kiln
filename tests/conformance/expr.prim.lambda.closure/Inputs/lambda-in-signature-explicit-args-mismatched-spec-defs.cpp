// The other unit of ../lambda-in-signature-explicit-args-mismatched-spec-two-units-run.cpp:
// it calls `f<long>`'s closure under the name clang 19 and 22 give it
// (an asm label), so the program links only if kiln defines the same name.
// A closure's call operator takes its object pointer first.
int f_long_op(const void *) __asm__("_ZZ1fPT0_T_ENKUlvE3_clEv");
int other_mismatched() { return f_long_op(nullptr); }
