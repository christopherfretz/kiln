// The symbols of template-id-signature-mangle-kiln-run.cpp's
// specializations under -fkiln-reflection-mangling=kiln.
#define SYM(s) __asm__(#s)
struct E {};
E d_at(int) SYM(_Z4d_atIiEDTtl1KILu6__infota2ATIT_E_2TTIPS2_EEEEES2_);
E d_vt(int) SYM(_Z4d_vtIiEDTtl1KILu6__infode2vtIT_EEEEES2_);
E d_g(int) SYM(_Z3d_gIiEDTtl1KILu6__infode1gIT_EEEEES2_);
E d_eq(int) SYM(_Z4d_eqIiEDTtl1KILu6__infodesr2nsEoneqIT_EEEEES2_);
E d_m(int) SYM(_Z3d_mIiEDTtl1KILu6__infodesr1SIT_EE1mIiEEEEES2_);
int p_at(int, E) SYM(_Z4p_atIiEiT_1KILu6__infota2ATIS0_E_2TTIPS0_EEE);
int p_vt(int, E) SYM(_Z4p_vtIiEiT_1KILu6__infode2vtIS0_EEE);
int p_g(int, E) SYM(_Z3p_gIiEiT_1KILu6__infode1gIS0_EEE);
int p_eq(int, E) SYM(_Z4p_eqIiEiT_1KILu6__infodesr2nsEoneqIS0_EEE);
int p_m(int, E) SYM(_Z3p_mIiEiT_1KILu6__infodesr1SIS0_EE1mIiEEE);
int p_nv(int, E) SYM(_Z4p_nvIiEiT_1KILu6__infodesr2nsE2nvIS0_EEE);
int p_f(int, E) SYM(_Z3p_fIiEiT_1KILu6__infota1FIS0_iE_2TTIS0_EEE);
int q_at(int) SYM(_Z4q_atIiQneLu6__infota2ATIT_E_2TTIPS1_EELS0_tyiEEiS1_);
int q_vt(int) SYM(_Z4q_vtIiQneLu6__infode2vtIT_EELS0_tyiEEiS1_);
int q_g(int) SYM(_Z3q_gIiQneLu6__infode1gIT_EELS0_tyiEEiS1_);
int q_eq(int) SYM(_Z4q_eqIiQneLu6__infodesr2nsEoneqIT_EELS0_tyiEEiS1_);
int q_m(int) SYM(_Z3q_mIiQneLu6__infodesr1SIT_EE1mIiEELS0_tyiEEiS1_);

int call_by_symbol() {
  d_at(1);
  d_vt(1);
  d_g(1);
  d_eq(1);
  d_m(1);
  return p_at(1, {}) + p_vt(1, {}) + p_g(1, {}) + p_eq(1, {}) + p_m(1, {}) +
         p_nv(1, {}) + p_f(1, {}) + q_at(1) + q_vt(1) + q_g(1) + q_eq(1) +
         q_m(1);
}
