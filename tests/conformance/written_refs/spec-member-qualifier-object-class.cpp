// The written-reference table (spec/ast.md §18.9.1), a member qualifier
// on a type-dependent object: the pattern's `acme` and `B` keep what the
// definition found (namespace `acme`), and each specialization whose
// object's class has its own `acme` gets rows naming what the
// instantiation bound ([basic.lookup.qual.general]/2-3): `W::acme`,
// `W::acme::B`, and `WB::f` for the member. A specialization whose class
// has no `acme` (`f<U>`) adds no qualifier rows. A component missing from
// the definition's namespace (`Only`) is a dependent entry resolved per
// specialization; its first component gets a row too.
//@ written-refs: spec-member-qualifier-object-class.refs
namespace acme { struct B { int f(); }; }
struct WB { int f(); };
struct W : acme::B, WB { struct acme { using B = WB; using Only = WB; }; };
struct U : acme::B {};
template <class T> int f(T t) { return t.acme::B::f(); }
template <class T> int g(T *p) { return p->acme::Only::f(); }
int x = f(W());
int y = f(U());
int z = g(static_cast<W *>(nullptr));
