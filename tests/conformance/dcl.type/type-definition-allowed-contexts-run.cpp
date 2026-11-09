// [dcl.type.general]/3, [dcl.fct]/17: the contexts where a class or
// enumeration definition IS allowed, each accepted as clang accepts it: a
// simple-declaration's decl-specifier-seq at namespace, block and class
// scope (with any declarators, typedef included), a for-init-statement, a
// non-template alias-declaration (also inside a class template), a member
// function's own class as its return type, local classes in member
// functions, and a lambda body anywhere (a default argument, an NTTP
// default, a template argument, a static member's initializer, a condition, a
// decltype). Elaborated references (no body) are fine in every context.
//@ std: c++20
//@ expect: run
//@ stdout: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16
#include <cstdio>
struct N1 { int v = 1; } n1, *pn1 = &n1;
typedef struct N2 { int v = 2; } T2;
using A3 = struct N3 { int v = 3; };
enum E4 { e4 = 4 } ev4 = e4;
struct M { struct In { int v = 5; } in; enum { m6 = 6 } m; M self() const { return *this; } };
template<class T> struct V { using U = struct L7 { int v = 7; }; U u; };
template<class T> struct W { W h() const { return *this; } int v = 8; };
struct Out { int f() const; static int s; };
int Out::f() const { struct Loc { int v = 9; }; return Loc{}.v; }
int Out::s = [] { struct L { int q = 10; }; return L{}.q; }();
int dflt(int x = [] { struct D { int v = 11; }; return D{}.v; }()) { return x; }
template<int N = [] { struct P { int v = 12; }; return P{}.v; }()> int nttp() { return N; }
template<int N> struct Z { static constexpr int v = N; };
struct S13; auto tr() -> struct S13 *; struct S13 { int v = 13; };
auto tr() -> struct S13 * { static S13 s; return &s; }
void elab(struct E14 *);
int main() {
  struct B { int v = 14; } b;
  int sum = tr()->v - 13;
  for (struct F { int i; } f{0}; f.i < 15; ++f.i) sum = f.i + 1;
  if (int c = [] { struct C { int v = 16; }; return C{}.v; }(); c == 16) {
    using Dt = decltype([] { struct Q { int v; }; return Q{0}; }());
    Dt dt{0};
    std::printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n", pn1->v, T2{}.v,
                A3{}.v, ev4, M{}.self().in.v, int(M::m6), V<int>{}.u.v, W<int>{}.h().v,
                Out{}.f(), Out::s, dflt(), nttp() + dt.v,
                Z<sizeof([] { struct G { char c; }; return G{}; }()) * 13>::v, b.v, sum, c);
  }
}
