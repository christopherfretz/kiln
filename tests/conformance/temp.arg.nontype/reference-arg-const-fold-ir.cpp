// [temp.arg.nontype], [expr.const]: a reference template parameter bound to
// a const integral variable with a constant initializer is emitted as the
// constant (clang `VisitSubstNonTypeTemplateParmExpr` folds the replacement
// through `tryEmitAsConstant`), while a dereference of a pointer parameter
// (`*p`) and of `&cg` stay loads in both clang 19 and 22. WP-998: kiln
// loaded `r` from the global. The folded bodies are the same in clang 19,
// but its module header is not (datalayout, `"frame-pointer"`), so the IR
// is compared with clang 22 only (WP-1157).
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
static const int cg = 3;
const int eg = 4;
static const long cl = 5;
enum E { e0, e1 };
static const E ce = e1;
template <const int *p> int deref() { return *p; }
template <const int &r> int ref() { return r; }
template <const long &r> long ref_long() { return r + 1; }
template <const E &r> int ref_enum() { return r; }
int f1() { return deref<&cg>(); }
int f2() { return deref<&eg>(); }
int f3() { return ref<cg>(); }
int f4() { return ref<eg>(); }
long f5() { return ref_long<cl>(); }
int f6() { return ref_enum<ce>(); }
int f7() { return *&cg; }
