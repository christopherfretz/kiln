// The definitions for ../expression-kinds-two-units-run.cpp, each under
// the name clang 19 and 22 give the specialization there.
template <unsigned long N> struct A {};
template <class... Z> struct L {};
static int calls;
int fp_pack(int, int) __asm__("_Z7fp_packIJiiEEN1AIXsZfp_EE4typeEDpT_");
int partial(A<2>) __asm__("_Z7partialIJcEEi1AIXsPiDpT_EEE");
int dq(int) __asm__("_Z2dqIiEDTclsr1S1gfp_EET_");
bool rw(int) __asm__("_Z2rwIiEDTcmfp_netl1XEtlS0_EET_");
int desig(int) __asm__("_Z5desigIiEDTcl1kildi1afp_di1bLi2EEEET_");
L<int, int> qh(int, long, int, int) __asm__(
    "_ZN1QIJilEE1hIJiiEEE1LIJDpDTcl2k2v110_SUBSTPACKfp_fp1_EEEEilDpT_");
int nd(int, int) __asm__("_ZN1N2ndIiEEiiT_");
int fp_pack(int, int) { return ++calls; }
int partial(A<2>) { return ++calls; }
int dq(int) { return ++calls; }
bool rw(int) { return ++calls; }
int desig(int) { return ++calls; }
L<int, int> qh(int, long, int, int) {
  ++calls;
  return {};
}
int nd(int, int) { return ++calls; }
int count() { return calls; }
