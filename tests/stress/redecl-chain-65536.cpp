// [basic.def]/2, [basic.link]/11 (redeclarations of one variable),
// [dcl.align]/6, spec/sema.md §5.2: 65,536 `extern int x;` redeclare one
// variable. Merging a redeclaration (alignas, visibility attributes,
// internal_linkage, the out-of-class static data member check) must not
// walk the whole chain each time: before WP-1076 this compile was
// quadratic and took minutes. Reduced from clang's
// Preprocessor/bigoutput.c, compiled as C++.
//@ expect: compile-ok
//@ strict-diags
//@ mem<=512
#define M0 extern int x;
#define M2  M0  M0  M0  M0
#define M4  M2  M2  M2  M2
#define M6  M4  M4  M4  M4
#define M8  M6  M6  M6  M6
#define M10 M8  M8  M8  M8
#define M12 M10 M10 M10 M10
#define M14 M12 M12 M12 M12
#define M16 M14 M14 M14 M14

M16

int x = 1;
int main() { return x - 1; }
