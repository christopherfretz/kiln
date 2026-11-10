// The symbols of template-id-signature-mangle-run.cpp's specializations
// that g++-16 spells the same way (`Dm`; spec/landed/abi/WP-1250.md).
#define SYM(s) __asm__(#s)
struct E {};
E d_at(int) SYM(_Z4d_atIiEDTtl1KILDmta2ATIPT_E_2TTIS2_EEEEES1_);
E d_vt(int) SYM(_Z4d_vtIiEDTtl1KILDmde2vtIT_EEEEES1_);
int p_at(int, E) SYM(_Z4p_atIiEiT_1KILDmta2ATIPS0_E_2TTIS2_EEE);
int p_vt(int, E) SYM(_Z4p_vtIiEiT_1KILDmde2vtIS0_EEE);
int q_at(int) SYM(_Z4q_atIiQneLDmta2ATIPT_E_2TTIS1_EELDmtyiEEiS0_);
int q_vt(int) SYM(_Z4q_vtIiQneLDmde2vtIT_EELDmtyiEEiS0_);
// `^^AT<T>` and `^^TT<T*>` are different reflections, so two overloads.
int p_ta_alias(int, E) SYM(_Z4p_taIiEiT_1KILDmta2ATIPS0_E_2TTIS2_EEE);
int p_ta_type(int, E) SYM(_Z4p_taIiEiT_1KILDmty2TTIPS0_EEE);

int call_by_symbol() {
  d_at(1);
  d_vt(1);
  return p_at(1, {}) + p_vt(1, {}) + q_at(1) + q_vt(1) + p_ta_alias(1, {}) +
         p_ta_type(1, {});
}
