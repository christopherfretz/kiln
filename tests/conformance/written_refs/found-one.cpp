// The found set of a dependent entry (spec/ast.md §18.12): ordinary
// unqualified lookup of a dependent call's name runs at the template
// definition ([temp.dep.candidate]/1, [basic.lookup.unqual]); the one
// declaration visible there is the set, open to argument-dependent
// lookup at each instantiation (`def=1+adl`). The specialization's row
// selects that element.
//@ written-refs: found-one.refs
void f(int);
template <class T> void one(T t) { f(t); }
void use() { one(1); }
