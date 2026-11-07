//@ mode: emit-llvm
// One constant table of 8000 elements, each a struct of nested structs
// holding 8 `{ const char *, size }` views (the shape of a generated
// name/metadata table): 64,000 pointer relocations in one constant byte
// image. Codegen lowers the image field by field,
// and every pointer field (and every `i64` or struct field, which may hold
// a member pointer) looked its relocation up with a linear scan of the
// image's whole relocation table: F fields x R relocations, quadratic in
// the table length (3.0 s here, 1.7 s at 6000 elements, over a 0.4 s
// frontend). The lookup binary-searches the offset-sorted table now
// (0.5 s; WP-665).
struct SV { const char *p; unsigned long n; };
struct Inner { SV a; SV b; SV c; unsigned long k; };
struct S { Inner i; Inner j; SV d; SV e; unsigned long x; unsigned long y; };
static constexpr const char *names[] = {
    "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10", "n11",
    "n12", "n13", "n14", "n15", "n16", "n17", "n18", "n19", "n20", "n21",
    "n22", "n23", "n24", "n25", "n26", "n27", "n28", "n29", "n30", "n31",
    "n32", "n33", "n34", "n35", "n36", "n37", "n38", "n39", "n40", "n41",
    "n42", "n43", "n44", "n45", "n46", "n47", "n48", "n49", "n50", "n51",
    "n52", "n53", "n54", "n55", "n56", "n57", "n58", "n59", "n60", "n61",
    "n62", "n63",
};
#define SV_(i, k) {names[((i) * (k)) % 64], k}
#define ROW(i)                                                                 \
  {{SV_(i, 1), SV_(i, 2), SV_(i, 3), i}, {SV_(i, 4), SV_(i, 5), SV_(i, 6), i},     \
   SV_(i, 7), SV_(i, 8), i, i},
#define R10(i)                                                                 \
  ROW(i##0) ROW(i##1) ROW(i##2) ROW(i##3) ROW(i##4) ROW(i##5) ROW(i##6)       \
  ROW(i##7) ROW(i##8) ROW(i##9)
#define R100(i)                                                                \
  R10(i##0) R10(i##1) R10(i##2) R10(i##3) R10(i##4) R10(i##5) R10(i##6)       \
  R10(i##7) R10(i##8) R10(i##9)
#define R1000(i)                                                               \
  R100(i##0) R100(i##1) R100(i##2) R100(i##3) R100(i##4) R100(i##5)           \
  R100(i##6) R100(i##7) R100(i##8) R100(i##9)
// Elements 1000..8999.
constexpr S table[] = {
    R1000(1) R1000(2) R1000(3) R1000(4) R1000(5) R1000(6) R1000(7) R1000(8)
};
static_assert(sizeof(table) / sizeof(table[0]) == 8000);
static_assert(table[7999].x == 8999 && table[0].j.c.n == 6);
const S *get(unsigned i) { return &table[i]; }
