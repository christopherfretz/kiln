// The found set (spec/ast.md §18.12) is fixed at the definition: an
// overload declared after the template is not an element and ordinary
// lookup at instantiation does not see it ([temp.dep.candidate]/1), so
// `later<int>` selects `f(long)`; an overload in the argument's
// namespace declared later is found by argument-dependent lookup in the
// instantiation context (the row is `how=adl`).
//@ written-refs: found-later-overload.refs
void f(long);
template <class T> void later(T t) { f(t); }
void f(int);
namespace lib {
struct A {};
void f(A);
} // namespace lib
void use() {
  later(1);
  later(lib::A{});
}
