// The other unit of ../lambda-in-signature-explicit-args-numbering-two-units-run.cpp:
// it calls the explicit specializations' closures under the names clang 19
// and 22 give them (asm labels), so the program links only if kiln defines
// the same names. A closure's call operator takes its object pointer first.
int ret2_int_op(const void *) __asm__("_ZZ4ret2vENKUlvE0_clEv");
int ret2_char_op(const void *) __asm__("_ZZ4ret2vENKUlvE2_clEv");
int other_explicit() { return ret2_int_op(nullptr) + ret2_char_op(nullptr); }
