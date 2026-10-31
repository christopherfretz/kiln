//@ expect: compile-ok
// [temp.constr.normal]: a concept-id in a concept's definition is
// normalized through its arguments.
template <class T> concept A = sizeof(T) > 1;
template <class T> concept B = A<T *> && A<T>;
template <class T> concept C = B<T> || __is_same(T, char);
static_assert(B<int>);
static_assert(C<char>);
static_assert(!C<bool>);
