// [temp.inst]/8, [expr.const]/4, [dcl.array]/3: a constexpr variable
// template specialization of unknown bound is instantiated when it is
// needed for constant evaluation, and its elements are read with the
// bound its initializer gives (also from a pack expansion and an
// explicit specialization with a bound of its own). clang 19 rejects the
// reads ("read of element of array without known bound"), a defect clang
// 22 fixed. WP-907.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
template <class T> constexpr T carr[] = {1, 2, 3};
static_assert(carr<int>[2] == 3);
static_assert(carr<long>[0] + carr<long>[1] == 3);
template <> constexpr int carr<char>[] = {7, 8, 9, 10};
static_assert(carr<char>[3] == 10 && sizeof(carr<char>) == 4 * sizeof(int));
template <class... T> constexpr unsigned long sizes[] = {sizeof(T)...};
static_assert(sizes<char, int>[0] == 1 && sizes<char, int>[1] == sizeof(int));
template <class T> constexpr char name[] = "kiln";
static_assert(name<int>[3] == 'n' && name<int>[4] == '\0');
