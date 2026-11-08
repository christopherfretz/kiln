// [dcl.attr.grammar]/5 [dcl.fct] [over.dcl] Calling-convention attributes
// on x86-64 as clang 19 (`handleCallConvAttr`, `checkCallingConvention`,
// `MergeFunctionDecl`, `CheckOverridingFunctionAttributes`): only a
// function type takes one; a redeclaration keeps the first explicit
// convention and a different one is an error (no new overload: the
// convention does not take part in `IsOverload`); argument counts and
// `pcs` strings are checked; conventions the target ignores warn; two
// conventions on one declaration conflict unless equal; an override must
// match; `regparm(N)` is an integer constant 0..6 and must agree across
// redeclarations; `no_caller_saved_registers` may be added only on the
// first declaration. spec/types.md §5.4, WP-808.
//@ expect: compile-fail
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 52:16 warning "'ms_abi' only applies to function types; type here is 'int'"
//@ diag: 53:23 warning "'ms_abi' attribute only applies to functions and methods"
//@ diag: 54:34 warning "'ms_abi' only applies to function types; type here is 'T'"
//@ diag: 59:30 error "function declared 'ms_abi' here was previously declared without calling convention"
//@ diag: 58:6 note "previous declaration is here"
//@ diag: 61:36 error "function declared 'preserve_all' here was previously declared 'preserve_most'"
//@ diag: 60:37 note "previous declaration is here"
//@ diag: 62:8 error "cannot initialize a variable of type 'void (*)()' with an lvalue of type 'void () __attribute__((ms_abi))'"
//@ diag: 64:16 error "'ms_abi' attribute takes no arguments"
//@ diag: 65:16 error "'preserve_most' attribute takes no arguments"
//@ diag: 66:16 error "'pcs' attribute takes one argument"
//@ diag: 67:16 error "invalid PCS type"
//@ diag: 69:16 warning "'stdcall' calling convention is not supported for this target"
//@ diag: 70:16 warning "'pcs' calling convention is not supported for this target"
//@ diag: 71:16 warning "'m68k_rtd' calling convention is not supported for this target"
//@ diag: 72:16 warning "'pascal' calling convention is not supported for this target"
//@ diag: 73:16 warning "'aarch64_vector_pcs' calling convention is not supported for this target"
//@ diag: 75:3 warning "unknown attribute '"
//@ diag: 77:3 warning "unknown attribute '"
//@ diag: 78:3 warning "unknown attribute '"
//@ diag: 80:24 error "preserve_most and ms_abi attributes are not compatible"
//@ diag: 82:24 error "cdecl and ms_abi attributes are not compatible"
//@ diag: 83:26 error "ms_abi and cdecl attributes are not compatible"
//@ diag: 89:21 error "virtual function 'v' has different calling convention attributes ('void ()') than the function it overrides (which has calling convention '"
//@ diag: 88:56 note "overridden virtual function is here"
//@ diag: 90:52 error "virtual function 'v' has different calling convention attributes ('"
//@ diag: 88:56 note "overridden virtual function is here"
//@ diag: 92:16 error "'regparm' parameter must be between 0 and 6 inclusive"
//@ diag: 93:16 error "'regparm' attribute takes one argument"
//@ diag: 96:34 error "function declared with regparm(3) attribute was previously declared with the regparm(2) attribute"
//@ diag: 95:6 note "previous declaration is here"
//@ diag: 97:16 warning "'regparm' only applies to function types; type here is 'int'"
//@ diag: 99:16 warning "'no_caller_saved_registers' only applies to function types; type here is 'int'"
//@ diag: 101:49 error "function declared with 'no_caller_saved_registers' attribute was previously declared without the 'no_caller_saved_registers' attribute"
//@ diag: 100:6 note "previous declaration is here"
//@ diag: 102:16 error "'no_caller_saved_registers' attribute takes no arguments"
// Subjects: a calling convention applies to a function type.
__attribute__((ms_abi)) int x;
struct __attribute__((ms_abi)) Cls {};
template<class T> __attribute__((ms_abi)) T var;
// Redeclarations keep the first explicit convention.
__attribute__((ms_abi)) void f();
void f();
void g();
__attribute__((ms_abi)) void g();
__attribute__((preserve_most)) void h();
__attribute__((preserve_all)) void h();
void (*p)() = f;
// Arguments.
__attribute__((ms_abi(1))) void k();
__attribute__((preserve_most(1))) void k2();
__attribute__((pcs)) void k3();
__attribute__((pcs("bogus"))) void k4();
// Conventions this target ignores, and spellings it lacks.
__attribute__((stdcall)) void s1();
__attribute__((pcs("aapcs"))) void s2();
__attribute__((m68k_rtd)) void s3();
__attribute__((pascal)) void s4();
__attribute__((aarch64_vector_pcs)) void s5();
[[gnu::ms_abi]] void q();
[[clang::ms_abi]] void q2();
[[clang::preserve_most]] void q3();
[[gnu::preserve_most]] void q4();
[[clang::regcall]] void q5();
// Two conventions on one declaration.
__attribute__((ms_abi, preserve_most)) void c1();
__attribute__((ms_abi)) __attribute__((ms_abi)) void c2();
__attribute__((ms_abi, sysv_abi)) void c3();
__attribute__((sysv_abi, ms_abi)) void c4();
// Variadic functions.
__attribute__((ms_abi)) void v1(...);
__attribute__((preserve_most)) void v2(...);
// An override keeps the overridden function's convention.
struct S { __attribute__((preserve_most)) virtual void v(); };
struct D : S { void v() override; };
struct D2 : S { __attribute__((preserve_all)) void v() override; };
// regparm(N): x86 only, an integer constant 0..6, part of the type.
__attribute__((regparm(7))) void r1();
__attribute__((regparm)) void r2();
__attribute__((regparm(2))) void r3();
void r3();
__attribute__((regparm(3))) void r3();
__attribute__((regparm(2))) int rx;
// no_caller_saved_registers: a function-type property too.
__attribute__((no_caller_saved_registers)) int nx;
void n1();
__attribute__((no_caller_saved_registers)) void n1();
__attribute__((no_caller_saved_registers(1))) void n2();
