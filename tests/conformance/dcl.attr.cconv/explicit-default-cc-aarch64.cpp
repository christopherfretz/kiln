// [dcl.fct] [over.dcl] [class.virtual]/18 A calling convention written
// as the target's default on `AArch64` (`cdecl`, `sysv_abi`, or an
// ignored one such as `stdcall` that warns and becomes the default) is
// explicit, as clang's `getCallingConvAttributedType`: a redeclaration
// with another explicit convention is an error naming it, and a second
// convention on the same declaration conflicts with it; an override
// keeps the default type. The function type is the same as without it
// (`__is_same`, conversions) and prints as written. The x86-64 forms are
// in the sibling -x86_64 tests. Reduced from real code. spec/types.md
// §5.4, WP-1858.
//@ expect: compile-fail
//@ args: --target=aarch64-linux-gnu
//@ strict-diags
//@ diag: 38:29 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 37:30 note "previous declaration is here"
//@ diag: 40:30 error "function declared 'ms_abi' here was previously declared 'cdecl'"
//@ diag: 39:29 note "previous declaration is here"
//@ diag: 42:21 error "function declared 'cdecl' here was previously declared 'preserve_most'"
//@ diag: 41:37 note "previous declaration is here"
//@ diag: 49:4 error "function declared 'cdecl' here was previously declared 'aarch64_vector_pcs'"
//@ diag: 48:42 note "previous declaration is here"
//@ diag: 52:32 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 51:30 note "previous declaration is here"
//@ diag: 54:16 warning "'stdcall' calling convention is not supported for this target"
//@ diag: 54:31 error "function declared 'cdecl' here was previously declared 'preserve_all'"
//@ diag: 53:36 note "previous declaration is here"
//@ diag: 55:16 warning "'pcs' calling convention is not supported for this target"
//@ diag: 56:39 error "function declared 'aarch64_sve_pcs' here was previously declared 'cdecl'"
//@ diag: 55:36 note "previous declaration is here"
//@ diag: 57:23 error "ms_abi and cdecl attributes are not compatible"
//@ diag: 58:24 warning "'vectorcall' calling convention is not supported for this target"
//@ diag: 58:24 error "cdecl and ms_abi attributes are not compatible"
//@ diag: 61:44 error "virtual function 'f' has different calling convention attributes ('void () __attribute__((cdecl))') than the function it overrides (which has calling convention 'void () __attribute__((aarch64_vector_pcs))')"
//@ diag: 60:61 note "overridden virtual function is here"
//@ diag: 67:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'void () __attribute__((cdecl))'"
// Redeclarations: a written default convention is explicit.
__attribute__((ms_abi)) void f1();
__attribute__((cdecl)) void f1();
__attribute__((cdecl)) void f2();
__attribute__((ms_abi)) void f2();
__attribute__((preserve_most)) void f3();
[[gnu::cdecl]] void f3();
void f4();
__attribute__((cdecl)) void f4();
__attribute__((cdecl)) void f5();
void f5();
typedef __attribute__((cdecl)) void F6();
__attribute__((aarch64_vector_pcs)) void f6();
F6 f6;
// A convention the target ignores becomes the default, written.
__attribute__((ms_abi)) void g1();
__attribute__((sysv_abi)) void g1();
__attribute__((preserve_all)) void g2();
__attribute__((stdcall)) void g2();
__attribute__((pcs("aapcs"))) void g3();
__attribute__((aarch64_sve_pcs)) void g3();
__attribute__((cdecl, ms_abi)) void a1();
__attribute__((ms_abi, vectorcall)) void a2();
// Overriding virtual functions.
struct B { virtual __attribute__((aarch64_vector_pcs)) void f(); virtual void g(); };
struct D : B { __attribute__((cdecl)) void f() override; __attribute__((cdecl)) void g() override; };
// The type is the type without the convention.
static_assert(__is_same(void(), F6));
static_assert(__is_same(void(*)(), void(__attribute__((cdecl)) *)()));
__attribute__((cdecl)) void k1();
void (*pk1)() = k1;
int bad1 = k1;
