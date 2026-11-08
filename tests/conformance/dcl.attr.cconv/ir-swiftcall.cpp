// [dcl.fct] [dcl.attr.grammar]/5 `swiftcall` and `swiftasynccall` lower
// as clang 19 (`swiftcc` / `swifttailcc` with clang's SwiftABI argument
// classification: no extension attributes, `__int128` whole, records
// expanded into their scalar fields, and the `U9swiftcall` /
// `U14swiftasynccall` mangling). spec/types.md §5.4, WP-808.
//@ expect: ir
//@ xfail(kiln): WP-808 follow-up, the swiftcall/swiftasynccall lowering (clang SwiftABI) is not implemented and both are a hard error
#define SW __attribute__((swiftcall))
#define SA __attribute__((swiftasynccall))
struct S8 { int a, b; };
struct F3 { float a, b, c; };
SW int s1(int a, char b, double c, void *p) { return a; }
SW S8 s2(S8 a) { return a; }
SW F3 s3(F3 a) { return a; }
SW __int128 s4(__int128 a) { return a; }
SA void s5(void *ctx) {}
typedef void (*SP)() SW;
void m(SP) {}
int use() { S8 x{1, 2}; return s1(1, 'a', 2.0, nullptr) + s2(x).a + int(s3({}).c) + int(s4(5)); }
