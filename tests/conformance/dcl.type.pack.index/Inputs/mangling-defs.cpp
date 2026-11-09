// The definitions for ../mangling-two-units-run.cpp, each under the name
// clang 19 and 22 give the specialization there.
int xf(int) __asm__("_ZN1XIJilEE1fIiEEDTplfp_tliEET_");
long xg(int) __asm__("_ZN1XIJilEE1gIiEElT_");
int xh(int, int, long) __asm__("_ZN1XIJilEE1hIiEEiT_il");
int pr(int, double) __asm__("_Z2prIJidEEDTEDpT_");
int pt(int, int, int) __asm__("_Z2ptIJiiEEiT_DpS0_");
int xf(int) { return 1; }
long xg(int) { return 2; }
int xh(int, int, long) { return 3; }
int pr(int, double) { return 4; }
int pt(int, int, int) { return 5; }
