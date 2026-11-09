// The definitions for ../lambda-default-arg-mangling-two-units-run.cpp,
// each under the name clang 19 and 22 give the specialization there.
int nm1() __asm__("_ZN1N1mINS_UlvE_EEEiv");
int nm2() __asm__("_ZN1N1mINS_UlvE0_EEEiv");
int rd() __asm__("_ZN1R1dINS_UliE_EEEiv");
int sm() __asm__("_ZN1SIiE1mINS0_UlvE_EEEiv");
int nm1() { return 1; }
int nm2() { return 2; }
int rd() { return 3; }
int sm() { return 4; }
