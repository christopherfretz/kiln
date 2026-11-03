// [WP-453] Constant folding of ext vector lane reads and `const` vector
// globals at -O0 (clang ScalarExprEmitter::VisitArraySubscriptExpr visits
// the vector base as a scalar, VisitDeclRefExpr's tryEmitAsConstant folds
// a `const` non-volatile vector variable, and the lane is an
// `extractelement` of the constant; ConstantEmitter::tryEmitPrivate folds
// a vector initializer with EvaluateAsRValue). A lane of a `constexpr` or
// `const` local or global vector folds for int, float and bool lanes (a
// non-constant index extracts from the constant, its side effects kept);
// a global initialized from a `const` vector gets a static initializer; a
// `const` global whose every use folds is not emitted. A volatile or
// non-const vector, a lane read through a pointer, a swizzle and an
// `extern` vector without an initializer stay loads.
typedef int i4 __attribute__((ext_vector_type(4)));
typedef float f4 __attribute__((ext_vector_type(4)));
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef long l3 __attribute__((ext_vector_type(3)));
int g() { constexpr i4 k = {1, 2, 3, 4}; return k[2]; }
float gf() { constexpr f4 k = {1, 2, 3, 4.5f}; return k[3]; }
bool gb() { constexpr b4 k = {true, false, true, false}; return k[2]; }
int gl() { const i4 k = {1, 2, 3, 4}; return k[1]; }
int gi(int i) { constexpr i4 k = {1, 2, 3, 4}; return k[2] + k[i] + (k)[1]; }
int gs(int i) { const i4 k = {1, 2, 3, 4}; int r = k[i++]; return r + i; }
float lanef(int i) { const f4 k = {1, 2, 3, 4}; return k[i]; }
bool laneb() { const b4 k = {true, false, true, false}; return k[0] && k[1]; }
int vol() { const volatile i4 k = {1, 2, 3, 4}; return k[1]; }
int nonconst() { i4 k = {1, 2, 3, 4}; return k[1]; }
int swz() { constexpr i4 k = {1, 2, 3, 4}; return k.y; }
int ptr() { const i4 k = {1, 2, 3, 4}; const i4 *p = &k; return k[1] + (*p)[2]; }
i4 whole() { const i4 k = {1, 2, 3, 4}; return k; }
const i4 gk = {1, 2, 3, 4};
int ggk(int i) { return gk[i]; }
extern const i4 ek;
int gek() { return ek[1]; }
const i4 cv = {5, 6, 7, 8};
i4 gv = cv;
i4 gadd = cv + 1;
int gcv = cv[1];
constexpr i4 ce = {9, 10, 11, 12};
int gce = ce[3];
const f4 cf = {1.5f, 2, 3, 4};
f4 gfv = cf * 2;
const b4 cb = {true, false, true, true};
b4 gbv = cb;
bool gbl = cb[1];
const l3 c3 = {1, 2, 3};
l3 g3 = c3;
long g3l = c3[2];
i4 nv = {1, 2, 3, 4};
i4 gnv = nv;
i4 gek2 = ek;
int gekl = ek[0];
