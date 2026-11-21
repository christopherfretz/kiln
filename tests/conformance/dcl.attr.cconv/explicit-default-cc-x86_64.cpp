// [dcl.fct] [over.dcl] [class.virtual]/18 A calling convention written
// as the target's default (`sysv_abi` and `cdecl` on x86-64, or an
// ignored one that warns and becomes the default) is explicit, as clang's
// `getCallingConvAttributedType`: a redeclaration with another explicit
// convention is an error naming it, and a second convention on the same
// declaration conflicts with it; an override keeps the default type. The
// function type is the same as without it (overloading, `__is_same`,
// template arguments, conversions) and prints as written. Reduced from
// real code. spec/types.md §5.4, WP-1858.
//@ expect: compile-fail
//@ args: --target=x86_64-linux-gnu
//@ strict-diags
//@ diag: 50:32 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 49:30 note "previous declaration is here"
//@ diag: 52:29 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 51:30 note "previous declaration is here"
//@ diag: 56:30 error "function declared 'ms_abi' here was previously declared 'cdecl'"
//@ diag: 55:32 note "previous declaration is here"
//@ diag: 63:4 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 62:30 note "previous declaration is here"
//@ diag: 65:22 error "function declared 'ms_abi' here was previously declared 'cdecl'"
//@ diag: 64:24 note "previous declaration is here"
//@ diag: 67:6 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 66:30 note "previous declaration is here"
//@ diag: 69:43 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 68:41 note "previous declaration is here"
//@ diag: 72:16 warning "'stdcall' calling convention is not supported for this target"
//@ diag: 72:31 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 71:30 note "previous declaration is here"
//@ diag: 74:34 error "function declared 'vectorcall' here was previously declared 'ms_abi'"
//@ diag: 73:30 note "previous declaration is here"
//@ diag: 76:16 warning "'pcs' calling convention is not supported for this target"
//@ diag: 76:36 error "function declared 'cdecl' here was previously declared 'preserve_most'"
//@ diag: 75:37 note "previous declaration is here"
//@ diag: 77:16 warning "'stdcall' calling convention is not supported for this target"
//@ diag: 78:30 error "function declared 'ms_abi' here was previously declared 'cdecl'"
//@ diag: 77:31 note "previous declaration is here"
//@ diag: 80:24 warning "'stdcall' calling convention is not supported for this target"
//@ diag: 80:24 error "cdecl and ms_abi attributes are not compatible"
//@ diag: 81:26 error "ms_abi and cdecl attributes are not compatible"
//@ diag: 83:16 error "ms_abi and cdecl attributes are not compatible"
//@ diag: 86:35 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 85:41 note "previous declaration is here"
//@ diag: 88:76 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 88:42 note "previous declaration is here"
//@ diag: 90:31 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 89:29 note "previous declaration is here"
// Redeclarations: a written default convention is explicit.
__attribute__((ms_abi)) void f1();
__attribute__((sysv_abi)) void f1();
__attribute__((ms_abi)) void f2();
__attribute__((cdecl)) void f2();
__attribute__((ms_abi)) void f3();
void f3();
__attribute__((sysv_abi)) void f4();
__attribute__((ms_abi)) void f4();
void f5();
__attribute__((sysv_abi)) void f5();
__attribute__((sysv_abi)) void f6();
void f6();
typedef __attribute__((sysv_abi)) void F7();
__attribute__((ms_abi)) void f7();
F7 f7;
[[gnu::sysv_abi]] void f8();
[[gnu::ms_abi]] void f8();
__attribute__((ms_abi)) void f9();
void f9() __attribute__((sysv_abi));
extern "C" __attribute__((ms_abi)) void c1();
extern "C" __attribute__((sysv_abi)) void c1();
// A convention the target ignores becomes the default, written.
__attribute__((ms_abi)) void g1();
__attribute__((stdcall)) void g1();
__attribute__((ms_abi)) void g2();
__attribute__((vectorcall)) void g2();
__attribute__((preserve_most)) void g3();
__attribute__((pcs("aapcs"))) void g3();
__attribute__((stdcall)) void g4();
__attribute__((ms_abi)) void g4();
// Two conventions on one declaration.
__attribute__((ms_abi, stdcall)) void a1();
__attribute__((sysv_abi, ms_abi)) void a2();
typedef __attribute__((sysv_abi)) void FS();
__attribute__((ms_abi)) FS a3;
// Members, block scope, builtins.
struct S { __attribute__((ms_abi)) void m(); void n(); };
__attribute__((sysv_abi)) void S::m() {}
__attribute__((sysv_abi)) void S::n() {}
void b1() { __attribute__((ms_abi)) void l1(); __attribute__((cdecl)) void l1(); }
__attribute__((ms_abi)) int abs(int);
__attribute__((sysv_abi)) int abs(int);
