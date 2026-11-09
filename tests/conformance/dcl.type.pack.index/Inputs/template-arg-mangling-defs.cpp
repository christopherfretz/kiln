// The definitions for ../template-arg-mangling-two-units-run.cpp, each
// under the name clang 22 gives the specialization there.
template <int N> struct S {};
static int calls;
int pa(S<3>) __asm__("_Z2paIJLi3ELi4EEEi1SIE");
S<2> yf_(int) __asm__("_ZN1YIJLi1ELi2EEE1fIiEE1SILi2EET_");
int pa(S<3>) { return 1; }
S<2> yf_(int) { calls = 2; return {}; }
int yf() { return calls; }
