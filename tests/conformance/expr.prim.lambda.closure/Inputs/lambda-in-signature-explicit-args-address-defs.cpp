// The other unit of ../lambda-in-signature-explicit-args-address-two-units-run.cpp:
// it calls the `long` specializations' closures under the names clang 19
// and 22 give them (asm labels), so the program links only if kiln defines
// the same names. A closure's call operator takes its object pointer first.
int fq_long_op(const void *) __asm__("_ZZ2fqT_ENKUlvE3_clEv");
int fp_long_op(const void *) __asm__("_ZZ2fpT_ENKUlvE4_clEv");
int ft_long_op(const void *) __asm__("_ZZ2ftT_ENKUlvE4_clEv");
int other_address() { return fq_long_op(nullptr) + fp_long_op(nullptr) + ft_long_op(nullptr); }
