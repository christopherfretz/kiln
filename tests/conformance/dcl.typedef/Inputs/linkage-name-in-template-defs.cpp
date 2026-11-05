// The definitions of the members ../linkage-name-in-template-run.cpp
// declares, under the names clang and GCC give them (asm labels), as a
// library built by another compiler would provide them.
int f_def(int, int) __asm__("_ZN1SILb0EE1fENS0_9DirectionENS0_8PositionE");
int f_def(int d, int p) { return d * 10 + p; }
int g_def(int *, int *) __asm__("_ZN1SILb0EE1gEPNS0_3RecEPNS0_2UnE");
int g_def(int *r, int *u) { return *r + *u; }
int mt_def(int, int) __asm__("_ZN1SILb0EE2mtIiEEiT_NS0_9DirectionE");
int mt_def(int u, int d) { return u * 10 + d; }
int h_def(int, int *) __asm__("_ZN1SILb0EE2InILi3EE1hENS2_1EEPNS2_1RE");
int h_def(int e, int *w) { return e * 10 + *w; }
int k_def(int) __asm__("_ZN1SILb0EE2MTIiE1kENS2_2QEE");
int k_def(int q) { return q; }
int m_def(int, int *) __asm__("_ZN1PIPiE1mENS1_2PEEPNS1_2PRE");
int m_def(int p, int *z) { return p * 10 + *z; }
int n_def(int) __asm__("_ZN1QIiE1nENS0_1EE");
int n_def(int q) { return q; }
int r_def(int) __asm__("_Z1rIiEN1SILb0EE9DirectionET_");
int r_def(int) { return 2; }
extern int vt_def __asm__("_Z2vtIN1SILb0EE9DirectionEE");
int vt_def = 9;
