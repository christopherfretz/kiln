// [class.temporary]/6, [except.ctor]: a temporary bound to a reference
// lives until the end of the reference's scope; its destructor runs at
// scope exit, and as an EH cleanup from the end of its full-expression
// (clang `pushLifetimeExtendedDestroy`: a deferred-deactivation EH
// cleanup for the full-expression plus a lifetime-extended cleanup
// guarded by an active flag). A temporary in a conditional branch saves
// its address and guards both cleanups with `cleanup.cond` flags. Inside
// a braced aggregate initialization the EH cleanups are deactivated once
// the aggregate is complete (clang `CleanupDeactivationScope`).
struct T { T(); T(int); ~T(); int v() const; };
int g(int);
void f1() { const T& r = T(); g(r.v()); }
void f2() { const T& r = T(); const T& s = T(1); g(r.v() + s.v()); }
void f3() { T&& r = T(g(0)); g(1); }
struct A { const T& t; int x; };
void f4() { A a{T(), g(2)}; g(a.x); }
void f5(int c) { const T& r = c ? T(1) : T(2); g(r.v()); }
void f6(int c) { const A& a = c ? A{T(1), g(1)} : A{T(2), g(2)}; g(a.x); }
void f6b(int c) { const A& a = c ? A{T(1), 1} : A{T(2), 2}; g(a.x); }
void f7(int c) { A a = c ? A{T(1), g(1)} : A{T(2), g(2)}; g(a.x); }
void f9(int c) { const int& r = c ? T(1).v() : g(3); g(r); }
struct B { T&& t; T&& u; };
void f10() { B b{T(1), T(g(0))}; g(0); }
void f11(int c) { const T& r = T(c ? T(1).v() : 0); g(r.v()); }
void f12() { A p{T(1), T().v()}; g(p.x); }
void f13() { A p{T(1), 2}; g(p.x); }
struct C { const T& t; T u; };
void h1() { A arr[2] = {{T(1), g(1)}, {T(2), g(2)}}; g(arr[0].x); }
void h2(int c) { A arr[2] = {{c ? T(1) : T(2), g(1)}, {T(2), g(2)}}; g(arr[1].x); }
void h3(int c) { const C& b = c ? C{T(1), T(2)} : C{T(3), T(4)}; g(b.u.v()); }
void h4() { const T (&r)[2] = {T(1), T(2)}; g(r[0].v()); }
void h5(int c) { const A& a = {c ? T(1) : T(g(2)), g(3)}; g(a.x); }
