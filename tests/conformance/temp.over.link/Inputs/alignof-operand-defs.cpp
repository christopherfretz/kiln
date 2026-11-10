// The definitions for ../alignof-operand-two-units-run.cpp, each under
// the name clang 19 and 22 give the specialization there.
int f_long(void *) __asm__(
    "_Z1fIlEiPN7storageIXstT_EXu11__alignof__N3msaIXstS1_EE4typeEEEE4typeE");
int g_int(void *) __asm__("_Z1gIiEiP1BIT_XatS1_EE");
int h_int(void *) __asm__("_Z1hIiEiP1BIT_Xplu11__alignof__S1_ELi1EEE");
int k_int(int, void *) __asm__("_Z1kIiEiT_P1BIS0_Xplu11__alignof__XfL0p_EEstS0_EE");
int f_long(void *) { return 0; }
int g_int(void *) { return 2; }
int h_int(void *) { return 3; }
int k_int(int, void *) { return 4; }
