// [expr.prim.id.dtor], [basic.lookup.qual.general]/4, [expr.ref]/4: the
// destructor named through a qualifier must be a member of the qualifier's
// class, which must be the object's class or a base; both type-names of a
// pseudo-destructor-name must denote the (scalar) object type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:9 error "no member named '~X' in 'W'"
//@ diag: 24:9 error "'X::~X' is not a member of class 'W'"
//@ diag: 25:9 error "no member named '~D' in 'W'"
//@ diag: 26:10 error "the type of object expression ('int') does not match the type being destroyed ('L' (aka 'long')) in pseudo-destructor expression"
//@ diag: 27:10 error "the type of object expression ('int') does not match the type being destroyed ('L' (aka 'long'))"
//@ diag: 27:6 error "the type of object expression ('int') does not match the type being destroyed ('L' (aka 'long'))"
//@ diag: 28:10 error "'Nope' does not refer to a type name in pseudo-destructor expression; expected the name of type 'int'"
//@ diag: 29:10 error "no member named '~Y' in namespace 'ns'"
//@ diag: 30:6 error "'Nope' does not refer to a type name in pseudo-destructor expression; expected the name of type 'int'"
struct W { ~W(); };
struct X { ~X(); };
struct D : W { ~D(); };
typedef int I;
typedef long L;
namespace ns { struct Y { ~Y(); }; }
void f(W* w, D* d, int* p, ns::Y* y) {
  w->W::~X();
  w->X::~X();
  d->W::~D();
  p->I::~L();
  p->L::~L();
  p->I::~Nope();
  y->ns::~Y();
  p->Nope::~I();
  y->ns::Y::~Y();
}
