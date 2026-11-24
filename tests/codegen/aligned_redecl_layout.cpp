// [dcl.align], GNU `aligned` on a redeclaration (WP-2830): a class's
// layout is its definition's, so `struct __attribute__((aligned(16))) C;`
// after `aligned(8)` leaves `alignof(C) == 8` and `sizeof(S) == 16`; a
// field typed with the 8-aligned typedef `R` keeps 8 after `R` is
// redeclared 16-aligned, while `alignof(R)` after it is 16; a variable's
// definition keeps its own alignment (`align 8`) and a load through the
// name after a stricter redeclaration uses the redeclaration's (16).
// Struct copies use the definitions' alignments. Compared with clang 19
// and clang 22 on both targets.
struct __attribute__((aligned(8))) C { int x; };
struct S { char c; C m; };
struct __attribute__((aligned(16))) C;

typedef int R __attribute__((aligned(8)));
struct T { char c; R r; };
typedef int R __attribute__((aligned(16)));

int v __attribute__((aligned(8))) = 1;
extern int v __attribute__((aligned(16)));

unsigned long size_s() { return sizeof(S); }
unsigned long align_c() { return alignof(C); }
unsigned long size_t_() { return sizeof(T); }
unsigned long align_r() { return alignof(R); }
void copy_s(S *d, const S *s) { *d = *s; }
void copy_t(T *d, const T *s) { *d = *s; }
C pass_c(C c) { return c; }
int load_v() { return v; }
