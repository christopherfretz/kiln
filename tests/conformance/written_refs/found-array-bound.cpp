// The found set of a dependent name in an array bound whose value is
// known but whose expression is instantiation-dependent (spec/ast.md
// §18.12, spec/types.md §7.2 "Kept bounds"): the bound stays on the
// type, so each specialization substitutes it and records the
// specialization's row for `f` (`spec=`), in a parameter's type and in
// a local's, beside the definition's set.
//@ written-refs: found-array-bound.refs
constexpr int f(int) { return 0; }
template <class T> int g(T, char (*)[sizeof(sizeof(f(T())))]) { return 1; }
template <class T> int k(T t) { char a[sizeof(sizeof(f(t)))]; return sizeof a; }
int u = g(1, nullptr) + k(1);
