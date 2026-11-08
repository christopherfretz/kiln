// clang `internal_linkage` and GNU `common` checks (WP-807; clang
// `mergeInternalLinkageAttr`, `MergeFunctionDecl` / `MergeVarDecl`,
// `handleCommonAttr`): a variable, function or class (else a warning:
// "and classes" with a serial comma for a non-variable, without for a
// parameter, variable template specialization or decomposition), no
// arguments, not on a variable with automatic storage (a warning at its
// name), on the first declaration of a function or variable (else an
// error, or after a definition a warning); it gives internal linkage, so
// `weak` conflicts with it. `common` is an error in C++, which rules out
// clang's `common` / `internal_linkage` exclusion.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 41:16 error "'internal_linkage' attribute takes no arguments"
//@ diag: 44:40 error "'internal_linkage' attribute does not appear on the first declaration"
//@ diag: 43:6 note "previous declaration is here"
//@ diag: 46:23 warning "attribute declaration must precede definition"
//@ diag: 45:5 note "previous definition is here"
//@ diag: 48:43 error "'internal_linkage' attribute does not appear on the first declaration"
//@ diag: 47:24 note "previous declaration is here"
//@ diag: 49:44 error "'internal_linkage' attribute does not appear on the first declaration"
//@ diag: 47:32 note "previous declaration is here"
//@ diag: 51:41 warning "'internal_linkage' attribute on a non-static local variable is ignored"
//@ diag: 57:16 warning "'internal_linkage' attribute only applies to variables, functions, and classes"
//@ diag: 58:26 warning "'internal_linkage' attribute only applies to variables, functions, and classes"
//@ diag: 59:21 warning "'internal_linkage' attribute only applies to variables, functions, and classes"
//@ diag: 61:28 warning "'internal_linkage' attribute only applies to variables, functions and classes"
//@ diag: 63:16 warning "'internal_linkage' attribute only applies to variables, functions and classes"
//@ diag: 64:29 warning "'internal_linkage' attribute only applies to variables, functions and classes"
//@ diag: 67:50 error "weak declaration cannot have internal linkage"
//@ diag: 68:16 error "weak declaration cannot have internal linkage"
//@ diag: 69:60 error "weak declaration cannot have internal linkage"
//@ diag: 77:23 error "'common' attribute is not supported in C++"
//@ diag: 78:16 error "'__common__' attribute is not supported in C++"
//@ diag: 79:16 error "'common' attribute takes no arguments"
//@ diag: 80:16 warning "'common' attribute only applies to variables"
//@ diag: 81:23 warning "'common' attribute only applies to variables"
//@ diag: 82:30 error "'common' attribute is not supported in C++"
//@ diag: 83:28 error "'common' attribute is not supported in C++"
//@ diag: 84:16 error "'common' attribute is not supported in C++"
__attribute__((internal_linkage(1))) int a1;
[[clang::internal_linkage]] void r1();
void r2();
__attribute__((internal_linkage)) void r2();
int r3;
extern __attribute__((internal_linkage)) int r3;
struct S2 { static int m; void mf(); };
__attribute__((internal_linkage)) int S2::m = 1;
__attribute__((internal_linkage)) void S2::mf() {}
void f() {
  __attribute__((internal_linkage)) int loc = 0;
  (void)loc;
  static int s __attribute__((internal_linkage));
  thread_local int tl __attribute__((internal_linkage));
  extern int e __attribute__((internal_linkage));
}
__attribute__((internal_linkage)) typedef int TD;
namespace __attribute__((internal_linkage)) NS {}
enum __attribute__((internal_linkage)) E {};
template <class T> int vt = 0;
template <> __attribute__((internal_linkage)) int vt<int> = 1;
struct D { int a, b; };
__attribute__((internal_linkage)) auto [da, db] = D{};
void p(int q __attribute__((internal_linkage)));
template <class T> struct TS {};
template <> struct __attribute__((internal_linkage)) TS<int> {};
__attribute__((internal_linkage)) __attribute__((weak)) void w1() {}
__attribute__((weak)) __attribute__((internal_linkage)) void w2() {}
__attribute__((internal_linkage)) void w3() __attribute__((weak));
__attribute__((internal_linkage)) int ok1;
__attribute__((internal_linkage)) void ok2();
__attribute__((internal_linkage)) void ok2() {}
struct __attribute__((internal_linkage)) OkC;
struct OkC {};
struct OkB;
struct __attribute__((internal_linkage)) OkB {};
int c1 __attribute__((common));
__attribute__((__common__)) int c2;
__attribute__((common(1))) int c4;
__attribute__((common)) void c5();
struct __attribute__((common)) C6 {};
void c7(int p __attribute__((common)));
void c8() { __attribute__((common)) int l; (void)l; }
__attribute__((common, internal_linkage)) int c9;
