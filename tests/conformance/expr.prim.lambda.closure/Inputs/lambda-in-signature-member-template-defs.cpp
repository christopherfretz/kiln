// The other unit of ../lambda-in-signature-member-template-return-two-units-run.cpp:
// it calls `S<int>::mret<int>` under the name clang 19 and 22 give it (an
// asm label), so the program links only if kiln defines the same name.
void mret_int(int) __asm__("_ZN1SIiE4mretIiEENS0_UlvE_ET_");
int other_member() { mret_int(1); return 5; }
