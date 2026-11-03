//@ threshold-time: 100
// A chain of 600 polymorphic classes, each deriving from the previous one
// (spec/sema-class.md §6.5, "Layout caches"; WP-428). Declaring a class's
// implicit virtual destructor looks up its deallocation function, which
// depends on the class's alignment ([expr.delete]/10), while the class is
// still being defined. That layout reuses the cached layouts of the
// complete bases (`Layouter::into_complete_cache`), so each class is laid
// out once. Laying out every base again for each class was cubic in the
// length: 3.8 s here, against 0.28 s now; clang takes about 1.1 s. The
// virtual lattice (virtual-lattice.cpp) takes the same path, but its
// virtual bases make clang exponential; a chain keeps the comparison
// meaningful. The threshold absorbs the shared host's noise (10-30%
// spread) and still fails the 13x of a per-class layout.

struct C { virtual ~C() {} long c; };
#define K(n, b) struct n : b { long m##n; };
#define K10(p, b) K(p##0, b) K(p##1, p##0) K(p##2, p##1) K(p##3, p##2) K(p##4, p##3) \
  K(p##5, p##4) K(p##6, p##5) K(p##7, p##6) K(p##8, p##7) K(p##9, p##8)
#define K100(p, b) K10(p##0, b) K10(p##1, p##09) K10(p##2, p##19) K10(p##3, p##29) \
  K10(p##4, p##39) K10(p##5, p##49) K10(p##6, p##59) K10(p##7, p##69) K10(p##8, p##79) \
  K10(p##9, p##89)
K100(S0, C) K100(S1, S099) K100(S2, S199) K100(S3, S299) K100(S4, S399) K100(S5, S499)
void f(C *c) { delete c; }
S599 last;
