// [except.spec]/7-8: an implicitly declared or defaulted special member is
// potentially throwing exactly when a subobject member it calls is (for a
// default constructor, also when a default member initializer can throw);
// a constructor does not look at subobject destructors and an abstract
// class's constructors skip its virtual bases. A destructor declared
// without a noexcept-specifier gets the implicit specification. Verified
// against clang 19.
//@ expect: compile-ok
int thr();
struct T { int a; };
struct Th { Th(); Th(const Th &); Th(Th &&); ~Th(); };
struct A { Th t; };
struct C { int x = thr(); };
struct C2 { int x = 1; };
struct F : Th {};
struct G : virtual T { T t2; };
struct M { mutable Th m; };
struct U { ~U(); };
struct W { U u; };
struct AB : virtual Th { virtual void f() = 0; AB(const AB &) = default; };
extern T t; extern A a; extern F f; extern G g; extern M m; extern W w;

static_assert(noexcept(T(t)));
static_assert(!noexcept(A(a)));
static_assert(!noexcept(F(f)));
static_assert(noexcept(G(g)));
static_assert(!noexcept(M(m)));
static_assert(noexcept(W(w)));
static_assert(!noexcept(C()));
static_assert(noexcept(C2()));
struct E { Th t; E() = default; };
static_assert(!noexcept(E()));
void use() {
  C c; C2 c2; W w2;
  (void)c; (void)c2; (void)w2;
}
