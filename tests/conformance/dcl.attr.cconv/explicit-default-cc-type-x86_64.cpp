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
//@ diag: 29:34 error "virtual function 'f' has different calling convention attributes ('void () __attribute__((sysv_abi))') than the function it overrides (which has calling convention 'void () __attribute__((ms_abi))')"
//@ diag: 27:49 note "overridden virtual function is here"
//@ diag: 31:32 error "virtual function 'h' has different calling convention attributes ('void () __attribute__((ms_abi))') than the function it overrides (which has calling convention 'void ()')"
//@ diag: 27:85 note "overridden virtual function is here"
//@ diag: 33:32 warning "'stdcall' calling convention is not supported for this target"
//@ diag: 33:47 error "virtual function 'f' has different calling convention attributes ('void () __attribute__((stdcall))') than the function it overrides (which has calling convention 'void () __attribute__((ms_abi))')"
//@ diag: 27:49 note "overridden virtual function is here"
//@ diag: 36:44 error "function declared 'cdecl' here was previously declared 'ms_abi'"
//@ diag: 36:44 note "previous declaration is here"
//@ diag: 52:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'void (*)() __attribute__((cdecl))'"
//@ diag: 53:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'void () __attribute__((cdecl))'"
//@ diag: 54:5 error "cannot initialize a variable of type 'int' with an lvalue of type 'void (*)() __attribute__((sysv_abi))'"
typedef __attribute__((sysv_abi)) void FS();
// Overriding virtual functions.
struct B { virtual __attribute__((ms_abi)) void f(); virtual void g(); virtual void h(); };
struct D : B {
  __attribute__((sysv_abi)) void f() override;
  __attribute__((sysv_abi)) void g() override;
  __attribute__((ms_abi)) void h() override;
};
struct D2 : B { __attribute__((stdcall)) void f() override; };
// Explicit specializations.
template <int N> __attribute__((ms_abi)) void ms() {}
template <> __attribute__((sysv_abi)) void ms<1>() {}
template <> void ms<2>() {}
template <int N> void df() {}
template <> __attribute__((sysv_abi)) void df<1>() {}
// The type is the type without the convention, printed as written.
static_assert(__is_same(void(), FS));
static_assert(__is_same(void(*)(), void(__attribute__((cdecl)) *)()));
template <class T> struct X { static const int v = 0; };
template <> struct X<void()> { static const int v = 1; };
static_assert(X<FS>::v == 1);
__attribute__((cdecl)) void k1();
void (*pk1)() = k1;
static_assert(__is_same(decltype(k1), void()));
void (__attribute__((cdecl)) *pk2)() = pk1;
void (__attribute__((sysv_abi)) *pk3)();
void (*pk4)() = (void (__attribute__((sysv_abi)) *)()) nullptr;
int bad1 = pk2;
int bad2 = k1;
int bad3 = pk3;
template <class F> void tf(F*) {}
void u() { tf(k1); tf(&k1); }
auto lam = []() __attribute__((sysv_abi)) {};
void (*plam)() = lam;
