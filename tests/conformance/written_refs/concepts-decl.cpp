// The written-reference table (spec/ast.md §18.4, stage 3), role
// `concept`: the concept name of a type-constraint ([temp.param]/2,
// [dcl.spec.auto.general]/1, [expr.prim.req.compound]/1), qualified
// or not, after the names in its template argument list. A concept-id in
// a requires-clause is an expression (stage 1) and keeps role `concept`.
//@ written-refs: concepts-decl.refs
namespace traits {
template <class T> concept Small = sizeof(T) <= 8;
template <class T, class U> concept Same = __is_same(T, U);
} // namespace traits
template <class T> concept Any = true;

template <traits::Small T> int first(T);
template <traits::Same<int> T> int second(T);
template <Any... Ts> int count(Ts...);
int third(Any auto x);
template <class T>
  requires traits::Small<T>
int fourth(T);
template <class T> concept Addable = requires(T a) {
  { a + a } -> traits::Same<T>;
};
auto local = [](traits::Small auto v) { return v; };
Any auto deduced = 1;
