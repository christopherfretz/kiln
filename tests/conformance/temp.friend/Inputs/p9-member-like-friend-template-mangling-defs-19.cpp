// The other unit of ../p9-member-like-friend-template-mangling-19-two-units-run.cpp:
// it calls the friend templates' specializations under clang 19's names.
int n1_ref(int, void *) __asm__("_ZN2n1IiEEiT_P1SQ1CIS0_E");
int n2_ref(int, void *) __asm__("_Z2n2IiEiT_P1SQ1CIS0_E");
int n3_ref(int) __asm__("_ZN2n3IiEEiT_Q1CIS0_E");
int g1_ref(int, void *) __asm__("_ZN2g1IiEEiT_P1AIiEQ1CITL0__E");
int g2_ref(int, void *) __asm__("_ZNF2g2IiEEiT_P1AIiEQ1CIS0_E");
int g3_ref(int, void *) __asm__("_ZF2g3IiQ1CIT_EEiS0_P1AIiE");
int other() {
  return n1_ref(0, nullptr) + n2_ref(0, nullptr) + n3_ref(0) + g1_ref(0, nullptr) +
         g2_ref(0, nullptr) + g3_ref(0, nullptr);
}
