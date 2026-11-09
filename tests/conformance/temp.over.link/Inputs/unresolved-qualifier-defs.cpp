// The definitions for ../unresolved-*-qualified-two-units-run.cpp, each
// under the name clang 19 and 22 give the specialization there.
struct A {};
int f1(int) __asm__("_Z2f1IiEDTclgs1gfp_EET_");
int f2(int) __asm__("_Z2f2IiEDTclgssr1NE1gfp_EET_");
int f3(int) __asm__("_Z2f3IiEDTclgssr1NE1hIiEfp_EET_");
int f4(int) __asm__("_Z2f4IiEDTplgssr1N1XIT_EE1vfp_ES0_");
int f5(int) __asm__("_Z2f5IiEDTclsr1N1ME1gfp_EET_");
int f6(int) __asm__("_Z2f6IiEDTclsr2NAE1gfp_EET_");
int f7(int) __asm__("_Z2f7IiEDTclsr1N1IE1qfp_EET_");
int f8(A) __asm__("_Z2f8I1AEDTclsrNT_1BE1gfp_EES1_");
int f9(int) __asm__("_Z2f9IiEDTclsr1N1XIT_E1YE1gfp_EES0_");
int f10(int) __asm__("_Z3f10IiEDTclsr1N1XIT_EE1gfp_EES0_");
int f11(A) __asm__("_Z3f11I1AEDTclsrNT_1ZIiEE1gfp_EES1_");
int f12(int) __asm__("_Z3f12IiEDTclsr1N1XIT_EE1gfp_EES0_");
int f13(int) __asm__("_Z3f13IN1N1XEiEDTclsrT_IT0_E1gfp_EES3_");
int f14(int) __asm__("_Z3f14IiEDTclsr2AXIT_EE1gplfp_Li1EEES0_");
int f1(int) { return 1; }
int f2(int) { return 2; }
int f3(int) { return 3; }
int f4(int) { return 4; }
int f5(int) { return 5; }
int f6(int) { return 6; }
int f7(int) { return 7; }
int f8(A) { return 8; }
int f9(int) { return 9; }
int f10(int) { return 10; }
int f11(A) { return 11; }
int f12(int) { return 12; }
int f13(int) { return 13; }
int f14(int) { return 14; }
