// [except.spec]/5 (C++14) is not checked under -fno-exceptions (clang
// `CheckExceptionSpecSubset`): the conversions the compile-fail sibling
// fnptr-exception-spec-compat-cxx14.cpp rejects are accepted.
//@ std: c++14
//@ strict-diags
//@ args: -fno-exceptions -Wno-c++17-compat-mangling
struct E1 {}; struct E2 {}; struct E3 : E1 {};
void f() throw(E1);
void g();
void h() noexcept;
void k() throw(E3);
void (*p1)() throw(E1) = f;
void (*p2)() throw() = g;
void (*p3)() noexcept = g;
void (*p4)() throw(E2) = f;
void (*p5)() throw(E1) = k;
void (*p6)() = h;
void (&r1)() noexcept = g;
struct S { void m(); };
void (S::*pm)() noexcept = &S::m;
void take(void (*)() noexcept);
void (*ret())() noexcept { return g; }
void t() {
  void (*q)() noexcept = h;
  q = g;
  take(g);
  void (*q3)() noexcept = (void (*)() noexcept)g;
  (void)q3;
}
void (*pp1)(void (*)() noexcept) = 0;
void ff(void (*)());
void (*pp2)(void (*)() noexcept) = ff;
void (*(*pp3)())() noexcept = 0;
void (*gg())();
void (*(*pp4)())() noexcept = gg;
struct B {}; struct D : B {}; struct P : private B {};
struct A1 : B {}; struct A2 : B {}; struct AA : A1, A2 {};
void d() throw(D); void dp() throw(D*); void dr() throw(D&);
void pr() throw(P); void aa() throw(AA); void ci() throw(const int*);
void two() throw(B, D);
void (*b1)() throw(B) = d, (*b2)() throw(B*) = dp, (*b3)() throw(void*) = dp;
void (*b4)() throw(B) = dr, (*b9)() throw(B&) = two;
void (*b5)() throw(B) = pr;
void (*b6)() throw(B) = aa;
void (*b7)() throw(int*) = ci;
void (*b8)() throw(const void*) = ci;
template <class T> void tf() throw(T);
void (*t1)() throw(int) = tf<int>, (*t2)() throw(int) = tf<long>;
auto lam = [] {};
void (*z)() noexcept = lam;
