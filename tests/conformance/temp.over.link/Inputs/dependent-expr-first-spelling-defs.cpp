// The definitions for ../dependent-expr-first-spelling-two-units-run.cpp,
// each under the name clang 19 gives the specialization there.
struct Empty {};
static int v13 = 13, v14 = 14;
int f(int) __asm__("_Z1fIiEN2EIIXsr1RIT_EE5valueES1_E4typeES1_");
int g(int) __asm__("_Z1gIiEN2EIIXsr1RIT_EE5valueES1_E4typeES1_");
int *g2(int) __asm__("_Z2g2IiEN2EIIXsr1RIT_vEE5valueEPS1_E4typeES1_");
int *f2(int) __asm__("_Z2f2IiEN2EIIXsr1RIT_vEE5valueEPS1_E4typeES1_");
int dm(Empty) __asm__("_Z2dmIiEi1AIXplsr1RIT_EE5valueLi1EEE");
int dg(int) __asm__("_Z2dgIiEiDTplsr1RIT_vEE5valueLi1EE");
int dh(int) __asm__("_Z2dhIiEiDTplsr1RIT_vEE5valueLi1EE");
int ak(Empty, int (*)[2]) __asm__("_Z2akIiEi1AILi0EEPAplsr1RIT_vEE5valueLi1E_i");
int al(int (*)[2]) __asm__("_Z2alIiEiPAplsr1RIT_vEE5valueLi1E_i");
int f(int x) { return x + 10; }
int g(int x) { return x + 10; }
int *g2(int) { return &v13; }
int *f2(int) { return &v14; }
int dm(Empty) { return 15; }
int dg(int) { return 16; }
int dh(int) { return 17; }
int ak(Empty, int (*)[2]) { return 19; }
int al(int (*)[2]) { return 17; }
