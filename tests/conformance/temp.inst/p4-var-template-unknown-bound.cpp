// [temp.inst]/4, [dcl.array]/3: a variable template specialization whose
// pattern is declared with an array of unknown bound takes its bound from
// the initializer, so naming it instantiates its definition: sizeof sees
// the complete array type. clang 19 and 22 agree.
//@ expect: compile-ok
//@ xfail(kiln): WP-879 follow-up, a variable template specialization of an array of unknown bound is not instantiated for sizeof (int[] stays incomplete)
template <class T> T arr[] = {1, 2};
static_assert(sizeof(arr<int>) == 2 * sizeof(int));
static_assert(sizeof(arr<char>) == 2);
