// [basic.lookup.qual.general]/4, [expr.prim.id.dtor], [expr.ref]/4,
// [class.virtual]/16: `p->N::TT::~TT()` and `p->N::T::~TT()` find the
// typedef `TT` in the qualifier's prefix `N`; `c->N::C<int>::~C<int>()`
// and `c->~C<int>()` (the template found in the object's class, also on a
// dependent `N::C<X>*`) name the destructor through a template-id. A
// qualified name calls the destructor non-virtually, an unqualified one
// virtually.
//@ expect: run
//@ strict-diags
//@ stdout: ~T
//@ stdout: ~T
//@ stdout: ~T
//@ stdout: ~U
//@ stdout: ~T
//@ stdout: ~C
//@ stdout: ~CD
//@ stdout: ~C
//@ stdout: ~CD
//@ stdout: ~C
//@ stdout: ~CD
//@ stdout: ~C
//@ stdout: ~C
extern "C" int puts(const char*);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
namespace N {
struct T { virtual ~T() { puts("~T"); } };
struct U : T { ~U() override { puts("~U"); } };
typedef T TT;
template <class X> struct C { virtual ~C() { puts("~C"); } };
template <class X> struct CD : C<X> { ~CD() override { puts("~CD"); } };
}
alignas(16) unsigned char buf[64];
template <class X> void g1(N::C<X>* c) { c->~C<X>(); }
template <class X> void g2(N::C<X>& c) { c.~C<X>(); }
int main() {
  N::T* p = new (buf) N::U;
  p->N::TT::~TT();
  p = new (buf) N::U;
  p->N::T::~TT();
  p = new (buf) N::U;
  p->::N::TT::~T();
  p = new (buf) N::U;
  p->~T();
  N::C<int>* c = new (buf) N::CD<int>;
  c->N::C<int>::~C<int>();
  c = new (buf) N::CD<int>;
  c->~C<int>();
  c = new (buf) N::CD<int>;
  g1(c);
  c = new (buf) N::CD<int>;
  g2(*c);
  c = new (buf) N::CD<int>;
  (*c).N::C<int>::~C<int>();
}
