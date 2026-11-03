// [dcl.array]/5 (an array of unknown bound is an incomplete type), [dcl.ref],
// [basic.align]: a pointer or reference to `int[]` is aligned as its element
// (clang `getTypeAlignInChars` of an incomplete array), so the parameter is
// `nonnull align 4` without `dereferenceable` and loads through it are
// `align 4`.
extern int arr[];
int (&ga)[] = arr;
int param(int (&a)[]) { return a[1]; }
int global() { return ga[2]; }
int pointer(int (*p)[]) { return (*p)[0]; }
int (&ret(int (&a)[]))[] { return a; }
