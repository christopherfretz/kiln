//@ expect: ast
//@ ast-implicit
//@ ast-filter: eager
//@ xfail(kiln): injected-class-names are not declared yet (spec/sema-class.md §2.2); the special members match
// [class.default.ctor]/1, [class.copy.ctor]/6, [class.copy.assign]/2,
// [class.dtor]/2: which implicit special members are declared at the end of
// an unreferenced class definition. Most are declared lazily on first lookup
// (as clang); a member needing overload resolution, a dynamic class, and a
// user-declared move operation make some eager. P1's copy members are
// declared by H's overload resolution, not by P1 itself (compare D1).
namespace eager {
struct A { int a; };
struct D1 { ~D1(); };
struct P1 { ~P1(); };
struct P2 { P2(const P2 &); };
struct P3 { P3(); };
struct M { mutable int m; };
struct V { virtual void f(); };
struct DV : V { };
struct H { P1 m; };
struct MC { MC(MC &&); };
struct MA { MA &operator=(MA &&); };
struct CA { CA &operator=(const CA &); };
}
