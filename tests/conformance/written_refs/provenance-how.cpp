// Lookup provenance (spec/ast.md §18.7.2): the `how` of each role.
// The dump prints `how=` only where it differs from what the line
// already shows (no entity: none; a `.`/`->` member: member; a `q=`:
// qualified; else unqualified), so `injected` and `builtin` appear and
// the defaults are checked by their absence.
//@ written-refs: provenance-how.refs
namespace ns {
struct Base {
  Base(int);
  int bm;
};
} // namespace ns
struct Derived : ns::Base {
  Derived() : Base(1), own(2) {}  // the base by its injected-class-name
  Derived(int) : Derived() {}     // delegating: the class's own name
  int own;
  Derived* self();                // the injected-class-name as a type
  int get() { return bm + Base::bm + Derived::own; }  // member in a base
};
template <class T> struct Tmpl {
  Tmpl<T>* next;                  // the injected-class-name as a template
  Tmpl* plain;
  T t;
};
struct Arrow {
  Derived* operator->();
};
struct Chain {
  Arrow operator->();
};
struct Gen {
  template <class U> int mt();
};
int use(Derived d, Derived* p, Chain c) {
  int r = d.own + p->own + c->own + d.bm + d.ns::Base::bm + ::ns::Base(1).bm;
  Gen gen;
  return r + __builtin_abs(r) + gen.template mt<int>();  // a member template
}
