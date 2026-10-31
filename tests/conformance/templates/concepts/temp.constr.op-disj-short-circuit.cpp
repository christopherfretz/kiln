//@ expect: compile-ok
// [temp.constr.op]/3: a disjunction is satisfied if its left operand is;
// the right operand is then not substituted into.
template <class T> concept Small = sizeof(T) == 1;
template <class T> concept C = Small<T> || T::value;
struct V { static constexpr bool value = true; };
struct W { static constexpr bool value = false; int a, b; };
static_assert(C<char>);
static_assert(C<V>);
static_assert(!C<W>);
