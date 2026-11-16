// The definitions for ../lambda-default-arg-member-nttp-mangling-run.cpp,
// each under the name clang 19 and 22 give the specialization there.
int m() __asm__("_ZN1SIiE1mINS0_UlvE_EEEiv");
int k1() __asm__("_ZN1SIiE1kILi1ETnDaXtlNS0_UlvE2_EEEEEiv");
int k2() __asm__("_ZN1SIiE1kILi2ETnDaXtlNS0_UlvE3_EEEEEiv");
int j1() __asm__("_ZN1SIiE1jILi1ENS0_UlvE1_EEEiv");
int j2() __asm__("_ZN1SIiE1jILi2ENS0_UlvE1_EEEiv");
int m() { return 1; }
int k1() { return 2; }
int k2() { return 3; }
int j1() { return 4; }
int j2() { return 4; }
