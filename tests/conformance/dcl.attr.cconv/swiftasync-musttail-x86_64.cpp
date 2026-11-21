// [dcl.fct] [dcl.attr.grammar]/5 `swiftasynccall` lowers as clang:
// `swifttailcc` with the Swift argument lowering, and in a
// `swiftasynccall` function a returned call (the return operand itself,
// no parentheses or conversions) of a `swiftasynccall` callee -- direct,
// through a pointer or a member call, not through a member pointer -- is
// emitted `musttail` and returned at once. `U14swiftasynccall`
// mangling. spec/abi.md §5.6, WP-1745 (WP-808).
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
#define SA __attribute__((swiftasynccall))
struct A { int a; double b; };
struct C { long a, b, c, d, e; };
struct M { SA void mf(void *); SA void mg(void *ctx) { return mf(ctx); } };
typedef float v4f __attribute__((vector_size(16)));
SA void g(void *ctx);
SA void h(void *ctx) { return g(ctx); }
SA void k(void *ctx) { g(ctx); }
SA int gi(int);
SA int hi(int x) { return gi(x); }
SA int hparen(int x) { return (gi(x)); }
SA void notail(void *c) { return (void)g(c); }
SA void viaptr(void (SA *q)(void *), void *c) { return q(c); }
SA void viamp(M *m, void (SA M::*f)(void *), void *c) { return (m->*f)(c); }
SA void member(M *m, void *c) { return m->mf(c); }
void m(void (SA *q)(void *)) { q(nullptr); }
SA A ga(int);
SA A ha(int x) { return ga(x); }
SA C gc(int);
SA C hc(int x) { return gc(x); }
SA v4f gv(int);
SA v4f hv(int x) { return gv(x); }
SA _Complex double gx(int);
SA _Complex double hx(int x) { return gx(x); }
A plain(int);
SA A hp(int x) { return plain(x); }
void use(M &mm) { mm.mg(nullptr); }
