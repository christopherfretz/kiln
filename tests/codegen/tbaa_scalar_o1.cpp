//@ args: -O1
// Type-based alias analysis metadata on scalar accesses at -O1 ([basic.lval]
// /11: an object is accessed only through a glvalue of a similar type, a
// signed/unsigned variant, or char / unsigned char / std::byte; clang
// `CodeGenTBAA`). Each load and store of an object of scalar type carries
// `!tbaa` `{T, T, 0}` under the "Simple C++ TBAA" root: a builtin is named
// for its signed variant, the narrow character types are "omnipotent char"
// (the root's only child); an enumeration with linkage is its `_ZTS` name,
// one in an unnamed namespace (and std::byte) is "omnipotent char"; a
// pointer or reference is "any pointer" (clang 19; clang 22 nodes per
// pointee, spec/codegen.md TBAA). A `bool` load also carries `!range` and
// `!noundef`. Parameter spills are tagged; the `this` spill load is not.
// `optnone` keeps the bodies.
#define ON __attribute__((optnone))
namespace std { enum class byte : unsigned char {}; }
enum En { A1, A2 };
enum class Ec : short { X };
namespace { enum Anon { Q }; }
ON int builtins(int* p, long* q, char* c, float* f, double* d, unsigned char* uc, signed char* sc,
                short* sh, unsigned short* us, long double* ld, wchar_t* wc, char8_t* c8,
                char16_t* c16, char32_t* c32, unsigned long long* ull) {
  *p = 1; *q = 2; *c = 3; *f = 4; *d = 5; *uc = 6; *sc = 7; *sh = 8; *us = 9; *ld = 1;
  *wc = 1; *c8 = 1; *c16 = 1; *c32 = 1; *ull = 1;
  return *p + *uc;
}
ON bool boolean(bool* b, bool c) { *b = c; return *b; }
ON int enums(En* e, Ec* ec, Anon* an, std::byte* by) {
  *e = A2; *ec = Ec::X; *an = Q; *by = std::byte{1};
  return *e;
}
struct S { int a; };
ON int pointers(int** pp, S** sp, void** vp, int*** ppp, const int** cp, void (**fp)(), int S::** mp) {
  *pp = 0; *sp = 0; *vp = 0; *ppp = 0; *cp = 0; *fp = 0; *mp = 0;
  return **pp;
}
ON int references(int& r, int*& rp, const int& cr) { r = 3; *rp = 4; return r + cr; }
int g;
ON int global() { g += 1; ++g; return g++; }
ON int bitcast(float* f) { return __builtin_bit_cast(int, *f); }
ON int conditional(bool c, int* a, int* b) { (c ? *a : *b) = 1; return c ? *a : *b; }
