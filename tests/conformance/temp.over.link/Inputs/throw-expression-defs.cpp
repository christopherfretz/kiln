// The definitions for ../throw-expression-two-units-run.cpp, each under
// the name clang 19 and 22 give the specialization there.
static int calls;
void s8(int) __asm__("_Z2s8IiEDTtwfp_ET_");
void s9(int) __asm__("_Z2s9IiEvT_");
int &s10(int) __asm__("_Z3s10IiEDTqufp_twfp_fp_ET_");
void s8(int) { ++calls; }
void s9(int) { ++calls; }
static int last;
int &s10(int t) { last = t; return last; }
int count() { return calls; }
