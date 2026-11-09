// The other unit of ../lambda-in-signature-address-nontemplate-match-two-units-run.cpp:
// it calls `fd<double>`'s closure under the name clang 19 and 22 give it
// (an asm label), so the program links only if kiln defines the same name.
// A closure's call operator takes its object pointer first.
int fd_double_op(const void *) __asm__("_ZZ2fdT_ENKUlvE0_clEv");
int other_nontemplate() { return fd_double_op(nullptr); }
