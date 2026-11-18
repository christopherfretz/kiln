// GNU `__restrict__` (GCC manual "Restricted Pointers"; WP-1663): clang
// `EmitFunctionProlog` marks an argument `noalias` when the definition's
// own parameter declaration (or `this` of a `__restrict` member
// function) is restrict-qualified and passed as a direct pointer:
// pointers, through a typedef or a template argument, a
// lambda's call operator, a constructor's parameter and a thunk. Not:
// a restrict nested below the top level, a declaration only (no prolog),
// a definition whose own declarator drops the qualifier, the return
// value, the call sites. Found running GCC's C++ testsuite. (A
// `__restrict` reference parameter is `noalias` too, for clang 19 and 22;
// kiln drops the qualifier from a reference type: WP-1663 remainder.)
void p1(int *__restrict a, int *__restrict__ b) {}
struct S { void m() __restrict; void n() const __restrict; void o(); int x; };
void S::m() __restrict {}
void S::n() const __restrict {}
void S::o() {}
void p4(int *__restrict *a) {}
void p5(int *const __restrict a) {}
void p6(int *__restrict a);
void p6(int *a) {}
void p7(int *a);
void p7(int *__restrict a) {}
void p8(int *__restrict a);
void use8(int *q) { p8(q); }
typedef int *__restrict RP;
void p9(RP a) {}
template <class T> void p10(T a) {}
template void p10<int *__restrict>(int *__restrict);
int *__restrict r11(int *__restrict a) { return a; }
void p12(int (*__restrict a)[4]) {}
struct Big { int a[10]; };
void p14(Big *__restrict b, Big c) {}
void p15(char *__restrict a, ...) {}
auto lam = [](int *__restrict a) { *a = 1; };
void use_lam(int *q) { lam(q); void (*fp)(int *) = lam; fp(q); }
struct D { D(int *__restrict p); int *q; };
D::D(int *__restrict p) : q(p) {}
struct A { virtual void f(int *__restrict p) __restrict; int a; };
struct B { virtual void g(); int b; };
struct C : B, A { void f(int *__restrict p) __restrict override; };
void C::f(int *__restrict p) __restrict {}
void A::f(int *__restrict p) __restrict {}
inline void inl(int *__restrict p) { *p = 1; }
void call(int *x) { inl(x); }
