// [dcl.fct]/1, [dcl.link]/6, [except.spec]/5, [over.load]/2: a file-scope
// declaration with C language linkage of a builtin's name redeclares the
// builtin (clang's implicit declaration, spec/sema.md §5.4): a different
// parameter list is "conflicting types" (with the builtin-type note), a
// potentially throwing exception specification does not match the
// builtin's `noexcept`, a custom-typechecked builtin cannot be redeclared
// at all and no builtin can be defined. A declaration in a namespace,
// with C++ linkage or with internal linkage is an ordinary function, and
// a call of the builtin's name after an invalid redeclaration calls the
// builtin. Return-type-only mismatches differ between clang 19 and 22:
// builtin-redecl-19.cpp / builtin-redecl-22.cpp (WP-410).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:19 error "conflicting types for '__builtin_sin'"
//@ diag: 27:19 note "'__builtin_sin' is a builtin with type 'double (double) noexcept'"
//@ diag: 28:17 error "conflicting types for '__builtin_trap'"
//@ diag: 28:17 note "'__builtin_trap' is a builtin with type 'void () __attribute__((noreturn)) noexcept'"
//@ diag: 29:16 error "cannot redeclare builtin function '__builtin_isnan'"
//@ diag: 29:16 note "'__builtin_isnan' is a builtin with type 'int (...) noexcept'"
//@ diag: 30:19 error "exception specification in declaration does not match previous declaration"
//@ diag: 30:19 note "previous declaration is here"
//@ diag: 32:19 error "exception specification in declaration does not match previous declaration"
//@ diag: 31:19 note "previous declaration is here"
//@ diag: 34:19 error "conflicting types for '__builtin_log'"
//@ diag: 33:19 note "'__builtin_log' is a builtin with type 'double (double) noexcept'"
//@ diag: 35:19 error "definition of builtin function '__builtin_tan'"
extern "C" double __builtin_sin(int);
extern "C" void __builtin_trap(int);
extern "C" int __builtin_isnan(...);
extern "C" double __builtin_atan(double) noexcept(false);
extern "C" double __builtin_cos(double);
extern "C" double __builtin_cos(double) noexcept(false);
extern "C" double __builtin_log(double);
extern "C" double __builtin_log(int);
extern "C" double __builtin_tan(double) { return 1; }

// Matching redeclarations: the builtin.
extern "C" double __builtin_exp(double);
extern "C" double __builtin_exp(double) noexcept;
extern "C" { double (__builtin_fabs)(double) throw(); }
extern "C" void *__builtin_memcpy(void *, const void *, unsigned long);

// Not the builtin's redeclaration.
double __builtin_acos(int);
namespace N { extern "C" double __builtin_sinh(int); }
static double __builtin_cosh(int);
extern "C" { static double __builtin_tanh(float); }

int a = __builtin_isnan(1.0) + __builtin_sin(0.5) + __builtin_log(1.0);
double b = __builtin_exp(0.0) + __builtin_fabs(-1.0) + __builtin_acos(1) + N::__builtin_sinh(1);
