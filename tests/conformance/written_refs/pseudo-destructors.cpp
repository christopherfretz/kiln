// The written-reference table (spec/ast.md §18.4), a pseudo-destructor
// name ([expr.prim.id.dtor]/2) on a scalar or dependent object: both
// type-names of `p->F::~Q()` name the type (role `type`, qualified by
// the nested-name-specifier before `F`, if any), `F` too, which is a
// type-name and not a nested-name-specifier; a simple-template-id's
// template name; with a dependent qualifier both are dependent.
//@ written-refs: pseudo-destructors.refs
namespace N {
typedef int I;
template <class T> using B = int;
} // namespace N
typedef int I;
struct C {
  typedef int J;
};
template <class T> using A = int;
void scalar(int *p) {
  p->~I();
  p->I::~I();
  p->C::J::~J();
  p->::I::~I();
  p->N::I::~I();
  p->A<int>::~A<int>();
  p->N::I::~B<int>();
}
template <class T> void dependent(T *t) {
  t->T::~T();
  t->~T();
}
template <class T> struct W {
  struct In {};
};
template <class T> void scoped(typename W<T>::In *t) { t->W<T>::In::~In(); }
void use(int *p) { dependent(p); }
