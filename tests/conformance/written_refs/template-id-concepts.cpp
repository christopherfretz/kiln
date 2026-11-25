// The written-reference table (spec/ast.md §18.5): a concept-id with
// dependent arguments names its concept ([temp.names]/2: the
// concept-name is not dependent) in each context it is written: a
// requires-clause, a nested requirement, a type-constraint, a
// constrained placeholder (`n::C auto`, whose first argument is the
// deduced type), a compound requirement's return-type-requirement and an
// expression, qualified and unqualified, nested in a variable
// template-id's arguments too.
//@ written-refs: template-id-concepts.refs
//@ std: c++20
namespace n {
template <class T> concept C = sizeof(T) > 0;
template <class T, class U> concept Same = __is_same(T, U);
template <bool B> constexpr bool b = B;
} // namespace n

template <class T>
  requires n::C<T>
int clause() { return 0; }
template <n::C T> int constraint() { return 0; }
template <class T> int placeholder(T t) {
  n::C auto y = t;
  return y;
}
template <class T> constexpr bool nested = requires(T t) {
  requires n::C<T *>;
  { t } -> n::Same<T>;
};
template <class T> constexpr bool expr = n::C<T> && n::b<n::C<T>>;
int z = clause<int>() + constraint<int>() + placeholder(1) + nested<int> + expr<int>;
