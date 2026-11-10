// The symbols of template-id-nondeduced-mangle-run.cpp's specializations
// that g++-16 spells the same way (`Dm`; spec/landed/abi/WP-1260.md):
// `ta <alias> [<aliased class specialization args>] _ <aliased type>`,
// the aliased type written with its dependent qualifier or decltype.
#define SYM(s) __asm__(#s)
struct E {};
E d_d(int) SYM(_Z3d_dIiEDTtl1KILDmta1D_N2RCIT_E4typeEEEEES2_);
int p_d(int, E) SYM(_Z3p_dIiEiT_1KILDmta1D_N2RCIS0_E4typeEEE);
int p_dptr(int, E) SYM(_Z6p_dptrIiEiT_1KILDmta1D_N2RCIPS0_E4typeEEE);
int q_d(int) SYM(_Z3q_dIiQneLDmta1D_N2RCIT_E4typeEELDmtyiEEiS1_);
int p_d2a(int, E) SYM(_Z5p_d2aIiEiT_1KILDmta2D2_N3RC2IS0_iE4typeEEE);
int p_d2b(int, E) SYM(_Z5p_d2bIiEiT_1KILDmta2D2_N3RC2IiS0_E4typeEEE);
int p_dpa(int, E) SYM(_Z5p_dpaIiEiT_1KILDmta2DPIN2RCIS0_E4typeEiE_3TT2IS4_iEEE);
int p_dpb(int, E) SYM(_Z5p_dpbIiEiT_1KILDmta2DPIiS0_E_3TT2IiS0_EEE);
int p_n1(int, E) SYM(_Z4p_n1IiEiT_1KILDmta2N1_N2RCI2TTIPS0_EE4typeEEE);
int p_n2(int, E) SYM(_Z4p_n2IiEiT_1KILDmta2N2IPN2RCIS0_E4typeEE_2TTIS5_EEE);
int p_n3(int, E) SYM(_Z4p_n3IiEiT_1KILDmta2N3_N2RCINS2_IS0_E4typeEE4typeEEE);
int p_x(int, E) SYM(_Z3p_xIiEiT_1KILDmta1XIN2RCIS0_E4typeEE_2TTIS4_EEE);
int p_z(int, E) SYM(_Z3p_zIiEiT_1KILDmta1Z_PN2RCIS0_E4typeEEE);
int p_dv(int, E) SYM(_Z4p_dvIiEiT_1KILDmta2DVIN2RCIS0_E4typeEE_2TTIS4_EEE);
int p_dn(E) SYM(_Z4p_dnILi3EEi1KILDmta2DN_N2RNIXT_EE4typeEEE);
int p_dt(int, E) SYM(_Z4p_dtIiEiT_1KILDmta2DT_DTtlS0_EEEE);
int p_nds(int, E) SYM(_Z5p_ndsIiEiT_1KILDmta3NDS_DTstS0_EEE);

int call_by_symbol() {
  d_d(1);
  return p_d(1, {}) + p_dptr(1, {}) + q_d(1) + p_d2a(1, {}) + p_d2b(1, {}) +
         p_dpa(1, {}) + p_dpb(1, {}) + p_n1(1, {}) + p_n2(1, {}) +
         p_n3(1, {}) + p_x(1, {}) + p_z(1, {}) + p_dv(1, {}) + p_dn({}) + p_dt(1, {}) + p_nds(1, {});
}
