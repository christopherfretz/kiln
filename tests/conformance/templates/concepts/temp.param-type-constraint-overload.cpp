//@ expect: compile-ok
// [temp.param]/4: a type-constraint `C T` introduces the constraint C<T>;
// an unsatisfied candidate is not viable ([over.match.viable]/3).
template <class T> concept Small = sizeof(T) <= 2;
template <class T> concept Large = sizeof(T) > 2;
template <Small T> constexpr int pick(T) { return 1; }
template <Large T> constexpr int pick(T) { return 2; }
static_assert(pick('c') == 1);
static_assert(pick(1L) == 2);
