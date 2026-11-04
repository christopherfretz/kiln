// GCC "Common Function Attributes", weakref / alias (clang
// handleWeakRefAttr, handleAliasAttr, Sema::CheckCompleteVariableDeclaration
// and ActOnStartOfFunctionDef): a weakref declaration must have internal
// linkage and appear at namespace scope, takes at most one string literal,
// and needs a target, its own or an `alias("x")` beside a bare `weakref`.
// An alias declaration cannot also be a definition, and a weak declaration
// cannot have internal linkage. The accepted shapes are libstdc++
// gthr-posix.h's `static __typeof(f) __gthrw_f __attribute__((__weakref__("f")))`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:28 error "weakref declaration must have internal linkage"
//@ diag: 31:35 error "weakref declaration must have internal linkage"
//@ diag: 32:35 error "weakref declaration of 'g3' must also have an alias attribute"
//@ diag: 33:35 error "'weakref' attribute takes no more than 1 argument"
//@ diag: 34:43 error "expected string literal as argument of 'weakref' attribute"
//@ diag: 35:50 error "definition 'g6' cannot also be an alias"
//@ diag: 39:45 error "weakref declaration of 'm' must be in a global context"
//@ diag: 40:41 error "weakref declaration of 'lv' must be in a global context"
//@ diag: 41:31 error "definition 'g10' cannot also be an alias"
//@ diag: 43:44 error "expected string literal as argument of 'weakref' attribute"
//@ diag: 45:36 error "weak declaration cannot have internal linkage"
//@ diag: 46:44 error "expected string literal as argument of 'weakref' attribute"
//@ diag: 47:42 error "expected string literal as argument of 'alias' attribute"
//@ diag: 50:24 error "definition 'g20' cannot also be an alias"
//@ diag: 51:36 error "'alias' attribute takes one argument"
// Not checked: clang's -Wgcc-compat "GCC does not allow 'weakref' attribute
// in this position on a function definition" (kiln has no -Wgcc-compat).

extern "C" int t1(int);
int g1(int) __attribute__((weakref("t1")));
extern int g2(int) __attribute__((weakref("t1")));
static int g3(int) __attribute__((weakref));
static int g4(int) __attribute__((weakref("t1", "t2")));
static int g5(int) __attribute__((weakref(t1)));
static int g6(int) __attribute__((weakref("t1"), alias("t1"))); // two targets
static int g7(int) __attribute__((weakref("t1")));
static int g8(int) __attribute__((weakref, alias("t1")));
namespace { int g9(int) __attribute__((weakref("t1"))); }
struct S { static int m(int) __attribute__((weakref("t1"))); };
void f() { static int lv __attribute__((weakref("t1"))); }
static int g10 __attribute__((weakref("t1"))) = 3;
namespace N { static int g12(int) __attribute__((weakref("t1"))); }
static int g14(int) __attribute__((weakref(1)));
static int g15 [[gnu::weakref("t1")]];
static int g16(int) __attribute__((weak, weakref("t1")));
static int g17(int) __attribute__((weakref(zz)));
static int g18(int) __attribute__((alias(1)));
extern int g19(int) __attribute__((alias("q")));
int q(int) { return 0; }
int g20 __attribute__((alias("q")));
static int g21(int) __attribute__((alias("a", "b")));
