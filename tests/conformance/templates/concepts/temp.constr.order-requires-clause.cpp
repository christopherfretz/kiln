//@ expect: compile-ok
// [temp.constr.order]: requires-clauses formed from the same concept-ids
// are compared through their normal forms.
template <class T> concept C1 = sizeof(T) > 1;
template <class T> concept C2 = sizeof(T) > 2;
template <class T> requires C1<T> constexpr int f(T) { return 1; }
template <class T> requires C1<T> && C2<T> constexpr int f(T) { return 2; }
static_assert(f(short()) == 1);
static_assert(f(1) == 2);
