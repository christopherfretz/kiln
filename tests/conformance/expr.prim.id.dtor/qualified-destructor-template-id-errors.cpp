// [basic.lookup.qual.general]/4, [expr.prim.id.dtor], [temp.names]/3: in
// `p->N::C::~Q`, `Q` is looked up in the prefix `N` of the qualifier
// (and the qualifier's class); a `~Q<...>` must name a template found
// there or in the object's class, and must name the object's class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:13 error "undeclared identifier 'Nope' in destructor name"
//@ diag: 31:12 error "no member named '~U' in 'N::T'"
//@ diag: 32:13 error "no member named '~U' in 'N::T'"
//@ diag: 33:17 error "no member named '~C' in 'N::C<int>'"
//@ diag: 34:6 error "no member named '~C' in 'N::C<int>'"
//@ diag: 35:18 error "destructor name 'Nope' does not refer to a template"
//@ diag: 36:7 error "no member named 'Nope' in 'N::C<int>'"
//@ diag: 37:16 error "undeclared identifier 'TT' in destructor name"
//@ diag: 45:7 error "'T' does not refer to a template"
//@ diag: 22:8 note "declared as a non-template here"
//@ diag: 46:13 error "destructor name 'T' does not refer to a template"
//@ diag: 47:7 error "'v' does not refer to a template"
//@ diag: 27:5 note "declared as a non-template here"
//@ diag: 48:18 error "destructor name 'v' does not refer to a template"
namespace N {
struct T { ~T(); };
struct U { ~U(); };
typedef T TT;
template <class X> struct C { ~C(); };
namespace M { struct V { ~V(); }; }
int v;
}
void f(N::T* p, N::C<int>* c, N::M::V* v) {
  p->N::T::~Nope();
  p->N::T::~U();
  p->N::TT::~U();
  c->N::C<int>::~C<long>();
  c->~C<long>();
  c->N::C<int>::~Nope<int>();
  c->~Nope<int>();
  v->N::M::V::~TT();
  p->N::TT::~TT();
  p->N::T::~TT();
  c->N::C<int>::~C<int>();
  c->~C<int>();
}
using namespace N;
void g(N::T* p, N::C<int>* c) {
  p->~T<int>();
  p->N::T::~T<int>();
  c->~v<int>();
  c->N::C<int>::~v<int>();
  c->C<int>::~C<int>();
}
