// [dcl.fct]/3: a parameter list of a lone unnamed non-dependent void
// (unqualified) means no parameters; void elsewhere in the list, a named
// void parameter or a cv-qualified lone void is ill-formed. A parameter
// declarator cannot be qualified ([dcl.meaning]/1).
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:12 error "'void' must be the first and only parameter if specified"
//@ diag: 22:7 error "'void' as parameter must not have type qualifiers"
//@ diag: 23:8 error "'void' as parameter must not have type qualifiers"
//@ diag: 24:12 error "'void' must be the first and only parameter if specified"
//@ diag: 25:17 error "'void' must be the first and only parameter if specified"
//@ diag: 26:23 error "'void' as parameter must not have type qualifiers"
//@ diag: 27:14 error "'void' must be the first and only parameter if specified"
//@ diag: 28:19 error "'void' must be the first and only parameter if specified"
//@ diag: 29:14 error "argument may not have 'void' type"
//@ diag: 30:11 error "'void' as parameter must not have type qualifiers"
//@ diag: 31:25 error "'void' must be the first and only parameter if specified"
//@ diag: 33:15 error "parameter declarator cannot be qualified"
//@ diag: 35:15 error "parameter declarator cannot be qualified"
void r(void, ...);
void s(void const);
void s2(const void);
void w(void, int);
void x(int, void);
typedef void V; void z(const V);
void w2(void a, int);
void w3(int, void b);
void w5(void a);
void (*fp)(void const);
void w10(int, const void);
namespace N { int k; }
void f(int N::k);
struct Q { static int m; };
void h(int Q::m = 0);
