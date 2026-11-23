// The found set (spec/ast.md §18.12) of a dependent call's name that
// ordinary lookup does not find at the definition: the lookup ran and
// found nothing (`def=0+adl`, not "no set"), so only argument-dependent
// lookup at instantiation can resolve it ([temp.dep.candidate]/1,
// [basic.lookup.argdep]); the row is `how=adl`.
//@ written-refs: found-none.refs
namespace lib {
struct A {};
void g(A);
} // namespace lib
template <class T> void none(T t) { g(t); }
void use() { none(lib::A{}); }
