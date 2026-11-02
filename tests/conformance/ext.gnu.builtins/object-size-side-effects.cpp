// __builtin_object_size of an operand with side effects or pointer casts
// in a constant expression (clang 19 tryEvaluateBuiltinObjectSize): the
// operand without its pointer casts is evaluated speculatively; a failure
// gives the default when the operand has side effects (calls included),
// and is otherwise not a constant expression ([expr.const]/10).
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:47 note "subexpression not valid in a constant expression"
//@ diag: 43:15 note "in call to 'g12()'"
struct S { int a; char c[8]; };
int ext();
constexpr int cx() { return 1; }
__attribute__((pure)) int pf();
// A side effect on an object from before the call fails the trial: the
// default (-1 for types 0 and 1, 0 for types 2 and 3), the side effect
// not performed.
constexpr long f1() { int a[4] = {}; int *p = a; long n = __builtin_object_size(p++, 0); return n * 100 + (p - a); }
static_assert(f1() == -100);
constexpr long f3() { int a[4] = {}; int i = 0; return __builtin_object_size(&a[i++], 2) * 100 + i; }
static_assert(f3() == 0);
// The left operand of a comma is ignored, its failure included.
constexpr long f4() { int a[4] = {}; return __builtin_object_size((a[0] = 1, a + 1), 0) * 100 + a[0]; }
static_assert(f4() == 1200);
constexpr long g9() { char b[6] = {}; return __builtin_object_size((ext(), b), 0); }
static_assert(g9() == 6);
// Pointer casts are looked through for the subobject (types 1 and 3).
constexpr long f2() { S s = {}; return __builtin_object_size((const char *)&s, 1) * 100 + __builtin_object_size((char *)&s.a, 3); }
static_assert(f2() == 1204);
constexpr long f5() { S s = {}; return __builtin_object_size((void *)&s.c[1], 1); }
static_assert(f5() == 7);
constexpr long f6() { int x = 0; return __builtin_object_size((char *)(long *)(void *)&x, 0); }
static_assert(f6() == 4);
// A call that is not a constant expression is a side effect (clang
// HasSideEffects), unless the callee is `const` or `pure`: then the
// operand is invalid.
constexpr long g10() { char b[6] = {}; return __builtin_object_size(&b[ext()], 0); }
static_assert(g10() == -1);
constexpr long g11() { char b[6] = {}; return __builtin_object_size(&b[cx()], 0); }
static_assert(g11() == 5);
constexpr long g12() { char b[6] = {}; return __builtin_object_size(&b[pf()], 0); }
static_assert(g12() == 0);
