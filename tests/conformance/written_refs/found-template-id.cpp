// The found set (spec/ast.md §18.12) of a function template-id with
// dependent arguments ([temp.names], [temp.dep.candidate]/1): both
// templates named `tid` are elements, open to argument-dependent lookup.
// `get<0>(t)` with no `get` visible is assumed to name a template
// ([temp.names]/3): the lookup found nothing (`def=0+adl`) and
// argument-dependent lookup at instantiation finds the namespace's
// template.
//@ written-refs: found-template-id.refs
template <class T> void tid(T);
template <class T, class U> void tid(T, U);
namespace lib {
struct A {};
template <int N> void get(A);
} // namespace lib
template <class T> void use_tid(T t) {
  tid<T>(t);
  get<0>(t);
}
void run() { use_tid(lib::A{}); }
