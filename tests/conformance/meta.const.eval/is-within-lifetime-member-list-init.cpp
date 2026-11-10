// [meta.const.eval]/3 (P2641R4), [basic.life]/1, [dcl.init.aggr]/7,
// [dcl.init.list]/4: a member list-initialized in a mem-initializer has its
// elements initialized in order. While one element's initializer is being
// evaluated, the elements before it are within their lifetime; that
// element, the ones after it (explicit, omitted or filled) are not; the
// member, and an aggregate element holding the current one, are. An
// element of class type initialized by a constructor is left to that
// constructor. clang 22 is the oracle.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-uninitialized
//@ xfail(gcc): GCC 16.1 has no __builtin_is_within_lifetime
#define L __builtin_is_within_lifetime

struct A { bool a, b; };
struct B { A x; consteval B() : x{L(&x.a), L(&x.b)} {} };
struct C { bool v[3]; consteval C() : v{true, L(&v[0]), L(&v[2])} {} };
struct D { A x; bool t; consteval D() : x{true, L(&x.a)}, t(L(&x.b)) {} };
struct E { A x; consteval E() : x{true, L(&x)} {} };
struct F { A x[2]; consteval F() : x{{true, L(&x[0])}, {L(&x[0]), L(&x[1].a)}} {} };
consteval bool fb() { B b; return !b.x.a && !b.x.b; }
static_assert(fb());
consteval bool fc() { C c; return c.v[1] && !c.v[2]; }
static_assert(fc());
consteval bool fd() { D d; return d.x.b && d.t; }
static_assert(fd());
consteval bool fe() { E e; return e.x.b; }
static_assert(fe());
consteval bool ff() { F f; return f.x[0].b && f.x[1].a && f.x[1].b; }
static_assert(ff());
struct G2 { A x; consteval G2() : x{L(&x), L(&x)} {} };
static_assert(G2().x.a);
static_assert(G2().x.b);
struct H { bool p[2]; consteval H() : p{L(&p[1]), L(&p[0])} {} };
static_assert(!H().p[0]);
static_assert(H().p[1]);
struct I { A x; consteval I() : x{L(&x.b), L(&x.b)} {} };
static_assert(!I().x.a);
static_assert(!I().x.b);
struct P { bool v[3]; consteval P() : v{L(&v[2])} {} };
static_assert(!P().v[0]);
struct Q { A x; consteval Q() : x{L(&x.b)} {} };
static_assert(!Q().x.a);
struct R { int k; bool c; };
struct S { R v[3]; consteval S() : v{{1, L(&v[2].c)}} {} };
static_assert(!S().v[0].c);
struct U { bool v[4]; consteval U() : v{L(&v[1]), L(&v[3])} {} };
static_assert(!U().v[0]);
static_assert(!U().v[1]);
struct Bs { bool z; };
struct DA : Bs { bool w; };
struct T1 { DA d; consteval T1() : d{{L(&d.w)}, L(&d.z)} {} };
static_assert(!T1().d.z); static_assert(T1().d.w);
struct Arr { bool e[2]; };
struct T2 { Arr r; consteval T2() : r{L(&r.e[1]), L(&r.e[0])} {} };
static_assert(!T2().r.e[0]); static_assert(T2().r.e[1]);
struct T3 { A x; consteval T3() : x(L(&x.b), L(&x.a)) {} };
static_assert(!T3().x.a); static_assert(T3().x.b);
struct N { bool a; bool b = __builtin_is_within_lifetime(&a); };
struct T4 { N n; bool q; consteval T4() : n{true}, q(true) {} };
static_assert(T4().n.b);
struct K { bool k; consteval K(const bool* p) : k(L(p)) {} };
struct KK { K u; K v; };
struct T5 { KK m; consteval T5() : m{K(&m.v.k), K(&m.u.k)} {} };
static_assert(!T5().m.u.k); static_assert(T5().m.v.k);
struct T6 { bool g[2][2]; consteval T6() : g{{true, L(&g[1][0])}, {L(&g[0][1]), L(&g[1][1])}} {} };
static_assert(!T6().g[0][1]); static_assert(T6().g[1][0]); static_assert(!T6().g[1][1]);
struct T7 { A x; A y; consteval T7() : x{true, L(&y)}, y{L(&x), L(&y)} {} };
static_assert(!T7().x.b); static_assert(T7().y.a); static_assert(T7().y.b);
