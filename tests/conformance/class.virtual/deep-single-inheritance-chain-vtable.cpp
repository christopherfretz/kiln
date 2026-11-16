// [class.virtual], [class.derived]: a chain of 400 classes, each deriving
// from the previous one, below a class with a virtual destructor. Every
// class is polymorphic and gets a vtable; nothing bounds the length of a
// single-inheritance chain. Found running kiln's compile-time benches.
//@ expect: run
//@ strict-diags
//@ xfail(kiln): WP-1511: vtable builder runs out of fuel (TooComplex) on a single-inheritance chain of about 300 classes
struct C { virtual ~C() {} };
#define K(n, b) struct n : b {};
#define K10(p, b) K(p##0, b) K(p##1, p##0) K(p##2, p##1) K(p##3, p##2) K(p##4, p##3) \
  K(p##5, p##4) K(p##6, p##5) K(p##7, p##6) K(p##8, p##7) K(p##9, p##8)
#define K100(p, b) K10(p##0, b) K10(p##1, p##09) K10(p##2, p##19) K10(p##3, p##29) \
  K10(p##4, p##39) K10(p##5, p##49) K10(p##6, p##59) K10(p##7, p##69) K10(p##8, p##79) \
  K10(p##9, p##89)
K100(S0, C) K100(S1, S099) K100(S2, S199) K100(S3, S299)
int main() { C *c = new S399; delete c; return 0; }
