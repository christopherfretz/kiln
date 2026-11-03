// [basic.start.static]/2-3: an initializer whose constant initialization
// fails (it reads non-constant `x` while `__builtin_is_constant_evaluated()`
// is true) is re-folded with `__builtin_is_constant_evaluated()` false;
// a scalar that folds becomes a static initializer, not a dynamic one
// (clang `ConstantEmitter::tryEmitPrivateForVarInit`). A const integral
// variable's uses fold to that value too (`use1`, `use2`, the local in
// `f2`; clang `VarDecl::evaluateValue`), and its unused internal global
// `cg` is not emitted. Folds through casts that clang's evaluator only
// continues past (a pointer that points at nothing) give static
// initializers as well; `__builtin_constant_p` of lax variables folds to
// true in codegen as in the Oven (spec/codegen.md "Static initialization").
int x = 3;
int g = __builtin_is_constant_evaluated() ? x : 7;
const int cg = __builtin_is_constant_evaluated() ? x : 9;
struct S { static const int m; };
const int S::m = __builtin_is_constant_evaluated() ? x : 11;
int use1() { return cg; }
int use2() { return S::m; }
int f() { static int s = __builtin_is_constant_evaluated() ? x : 5; return s; }
int f2() { const int l = __builtin_is_constant_evaluated() ? x : 6; return l; }
double dd = __builtin_is_constant_evaluated() ? x : 1.5;
int *p = __builtin_is_constant_evaluated() ? &x : nullptr;
thread_local int t = __builtin_is_constant_evaluated() ? x : 4;
int y = x;
constexpr int n = 4;
constexpr const void *vp = &n;
int g2 = (const int *)vp != nullptr;
long h = (long)(int *)8;
int k = reinterpret_cast<const char *>(&n) + 1 != nullptr;
int f3() { static int s3 = (int *)8 == (int *)8; return s3; }
const double d0 = 2.5;
const double d1 = d0 * 2;
constexpr const double *p1 = &d1;
int bcp() { return __builtin_constant_p(d1) + 2 * __builtin_constant_p(*p1) + 4 * __builtin_constant_p(cg); }
