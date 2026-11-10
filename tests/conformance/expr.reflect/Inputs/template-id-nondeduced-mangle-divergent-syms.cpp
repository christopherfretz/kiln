// The symbols of template-id-nondeduced-mangle-divergent-run.cpp's
// specializations in kiln's -fkiln-reflection-mangling=gcc spelling,
// which keeps the alias's own arguments where g++-16's
// (`ta2ND_i`, `ta2DLIN2RCIS0_E4typeEiE_…`, `ta2DPIiS0_E_…`) loses one.
#define SYM(s) __asm__(#s)
struct E {};
int p_nd(int, E) SYM(_Z4p_ndIiEiT_1KILDmta2NDIS0_E_iEE);
int p_dl(int, E) SYM(_Z4p_dlIiEiT_1KILDmta2DLIS0_iE_3TT2IN2RCIS0_E4typeEiEEE);
int p_dpc(int, E) SYM(_Z5p_dpcIiEiT_1KILDmta2DPIKiS0_E_3TT2IiS0_EEE);

int call_by_symbol() { return p_nd(1, {}) + p_dl(1, {}) + p_dpc(1, {}); }
