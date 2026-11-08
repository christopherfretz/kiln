// WP-813: the pointer-fact attributes: assume_aligned (the call's
// `align N`, or with an offset an `llvm.assume` "align" bundle, at -O0
// too, CGCall.cpp AssumeAlignedAttrEmitter), alloc_align / alloc_size, align_value
// (`align 64` on the parameter), callback (`!callback` metadata),
// noescape (`nocapture`), maybe_undef (the argument is `freeze`d), nonnull and
// returns_nonnull. spec/landed/codegen/WP-813.md.
void *__attribute__((assume_aligned(32))) aa();
void *__attribute__((assume_aligned(32, 8))) aa2();
void *__attribute__((alloc_align(1))) al(unsigned long);
void *__attribute__((alloc_size(1))) as(unsigned long);
void *__attribute__((alloc_size(1,2))) as2(unsigned long, unsigned long);
typedef double *__attribute__((align_value(64))) aligned_double;
void av(aligned_double p) { *p = 1; }
void av2(double *__attribute__((align_value(64))) p) { *p = 1; }
void cbk(void (*)(void*), void*) __attribute__((callback(1, 2)));
void ne(__attribute__((noescape)) int *p);
void mu(__attribute__((maybe_undef)) int p);
void nn(int *p, int *q) __attribute__((nonnull(1)));
void nn2(int *p, int *q) __attribute__((nonnull));
void nn3(__attribute__((nonnull)) int *p);
int *rn() __attribute__((returns_nonnull));
void ff(void*);
void use(int x) {
  ff(aa()); ff(aa2()); ff(al(64)); ff(as(4)); ff(as2(4,8));
  cbk(ff, 0); int i; ne(&i); mu(x); nn(&i,&i); nn2(&i,&i); nn3(&i); ff(rn());
}
void nndef(int *p, int *q) __attribute__((nonnull(1)));
void nndef(int *p, int *q) {}
int *rndef() __attribute__((returns_nonnull));
int *rndef() { static int z; return &z; }
void nedef(__attribute__((noescape)) int *p) {}
void *__attribute__((assume_aligned(32))) aadef() { return 0; }
