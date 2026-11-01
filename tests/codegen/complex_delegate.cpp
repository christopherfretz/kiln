// _Complex through delegating thunks (lambda invoker, inheriting
// constructor, this-adjusting virtual thunk), statement expressions and
// volatile globals (WP-211).
using C = _Complex double;
using CF = _Complex float;
C (*fp())(C) { return [](C x) { return x * x; }; }
struct B { B(C c, CF f) : v(c + f) {} C v; };
struct D : B { using B::B; };
D mkd(C c, CF f) { return D(c, f); }
struct X { virtual C f(C c) { return c; } };
struct Y { virtual C f(C c) { return -c; } int y; };
struct Z : X, Y { C f(C c) override { return c + 1.0; } };
C callz(Z& z, C c) { return z.f(c); }
C stmt(C a) { return ({ C t = a * 2.0; t; }); }
volatile C vg;
C vread() { return vg + vg; }
CF vcf(volatile CF* p) { ++*p; return *p; }
