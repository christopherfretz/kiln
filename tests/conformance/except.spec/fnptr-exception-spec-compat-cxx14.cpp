// [except.spec]/5 (C++14): when a pointer to function, reference to
// function or pointer to member function is initialized, assigned,
// passed or returned, the target's exception specification must allow
// every exception the source's allows; function types among the return
// and parameter types must have equivalent specifications
// ([except.spec]/3). Explicit casts are not checked.
//@ std: c++14
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0 -Wno-c++17-compat-mangling
//@ diag: 31:24 error "target exception specification is not superset of source"
//@ diag: 32:25 error "target exception specification is not superset of source"
//@ diag: 33:26 error "target exception specification is not superset of source"
//@ diag: 36:25 error "target exception specification is not superset of source"
//@ diag: 38:28 error "target exception specification is not superset of source"
//@ diag: 40:35 error "target exception specification is not superset of source"
//@ diag: 43:7 error "target exception specification is not superset of source"
//@ diag: 44:8 error "target exception specification is not superset of source"
//@ diag: 50:36 error "exception specifications of argument types differ"
//@ diag: 53:31 error "exception specifications of return types differ"
//@ diag: 63:25 error "target exception specification is not superset of source"
//@ diag: 64:25 error "target exception specification is not superset of source"
//@ diag: 65:28 error "target exception specification is not superset of source"
//@ diag: 68:57 error "target exception specification is not superset of source"
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
// Dynamic specifications: each type the source may throw must be caught
// by a handler for one the target lists ([except.handle]/3).
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
// The result of a user-defined conversion is not checked.
auto lam = [] {};
void (*z)() noexcept = lam;
