// [class.virtual], [lex.phases]/1.9 (WP-873): `-S -emit-llvm -flto=thin -fsplit-lto-unit` emits
// what clang's does: `EnableSplitLTOUnit` 1 under ThinLTO, the module summary, and the
// `!type` metadata on each vtable (one entry per address point and per
// virtual function, `.virtual` member-pointer ids, distinct ids for
// internal types, construction vtables in the VTT carrying it twice).
// C++23: in C++26 `X x` is constant-initialized (P3533), a separate gap
// (tests/conformance/class.mi/shared-virtual-base-constant-init-run.cpp).
//@ args: -std=c++23 -flto=thin -fsplit-lto-unit
struct A { virtual int f() const; virtual void g() noexcept; int a; };
struct B { virtual ~B(); virtual int h(int) &; int b; };
struct C : A, B { int f() const override; int h(int) & override; };
C c;
int C::f() const { return 1; }
int C::h(int) & { return 2; }
struct V { virtual void v(); };
struct M1 : virtual V { virtual void m1(); };
struct M2 : virtual V { virtual void m2(); };
struct X : M1, M2 { void v() override; };
void X::v() {}
X x;
namespace {
struct I { virtual int i(); };
int I::i() { return 3; }
struct J : I { int i() override { return 4; } };
}
int use() { J j; I* p = &j; return p->i(); }
struct Local;
struct P { virtual void p(Local*); };
void P::p(Local*) {}
