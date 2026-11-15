// -fsanitize=pointer-overflow in C: clang 19 treats `nullptr + 0` as
// undefined in C, so a zero offset is checked too and both the base and
// the result must be non-null (an `and` of the two tests); clang 20 and
// later (N3322) check as in C++: no check of a zero offset, and the two
// pointers must be both null or both non-null (an `icmp eq`). kiln
// follows -fclang-compat. spec/codegen.md "Sanitizers".
//@ expect: ir
//@ args: -std=gnu17 -fsanitize=pointer-overflow
int f1(int *p, long i) { return p[i]; }
int f2(int *p) { return p[0] + p[1]; }
int f3(int n) { int a[n]; a[0] = 1; return a[n - 1]; }
struct F { int n; int d[]; };
int f4(struct F *p, int i) { return p->d[i]; }
char *f5(char *p, unsigned u) { return p + u; }
