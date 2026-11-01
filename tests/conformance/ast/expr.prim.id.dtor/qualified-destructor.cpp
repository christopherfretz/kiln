// [expr.prim.id.dtor], [basic.lookup.qual.general]/4, [expr.ref]/4: a
// qualified destructor name on a class object names the destructor as a
// qualified member (`MemberExpr .~W`); on a scalar object `T::~T` and
// `S::T::~T` are pseudo-destructors; in a template the dependent form is
// a CXXPseudoDestructorExpr (instantiated in
// expr.prim.id.dtor/run-qualified-destructor.cpp).
//@ expect: ast
struct W { ~W(); };
struct U { W w; };
struct B { virtual ~B(); };
struct D : B { ~D(); };
typedef int I;
struct C { typedef int J; };
void f(U& u, D* d, int* p, W* w) {
  u.w.W::~W();
  d->B::~B();
  d->D::~D();
  w->W::~W();
  p->I::~I();
  p->C::J::~J();
  p->::I::~I();
}
template <class T> void g(T* t) { t->T::~T(); }
