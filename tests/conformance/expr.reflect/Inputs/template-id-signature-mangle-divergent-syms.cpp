// The symbols of template-id-signature-mangle-divergent-run.cpp's
// specializations in kiln's -fkiln-reflection-mangling=gcc spelling,
// where g++-16's drops the template arguments (`ft1g`, `ft2nseq`), the
// qualifier (`de2nvIS0_E`) or an alias argument (`ta1FIS0_E_...`).
#define SYM(s) __asm__(#s)
struct E {};
E d_g(int) SYM(_Z3d_gIiEDTtl1KILDmde1gIT_EEEEES1_);
E d_eq(int) SYM(_Z4d_eqIiEDTtl1KILDmdesr2nsEoneqIT_EEEEES1_);
E d_m(int) SYM(_Z3d_mIiEDTtl1KILDmdesr1SIT_EE1mIiEEEEES1_);
int p_g(int, E) SYM(_Z3p_gIiEiT_1KILDmde1gIS0_EEE);
int p_eq(int, E) SYM(_Z4p_eqIiEiT_1KILDmdesr2nsEoneqIS0_EEE);
int p_m(int, E) SYM(_Z3p_mIiEiT_1KILDmdesr1SIS0_EE1mIiEEE);
int p_nv(int, E) SYM(_Z4p_nvIiEiT_1KILDmdesr2nsE2nvIS0_EEE);
int p_f(int, E) SYM(_Z3p_fIiEiT_1KILDmta1FIS0_iE_2TTIS0_EEE);
int q_g(int) SYM(_Z3q_gIiQneLDmde1gIT_EELDmtyiEEiS0_);
int q_eq(int) SYM(_Z4q_eqIiQneLDmdesr2nsEoneqIT_EELDmtyiEEiS0_);
int q_m(int) SYM(_Z3q_mIiQneLDmdesr1SIT_EE1mIiEELDmtyiEEiS0_);

int call_by_symbol() {
  d_g(1);
  d_eq(1);
  d_m(1);
  return p_g(1, {}) + p_eq(1, {}) + p_m(1, {}) + p_nv(1, {}) + p_f(1, {}) +
         q_g(1) + q_eq(1) + q_m(1) - 1;
}
