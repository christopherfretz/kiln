// [meta.unary.prop] is_assignable, [dcl.fct]/6: `declval<U>()` for an
// abominable function type U (`void() const`, `void() &`) is ill-formed
// (no reference to it can be formed), so is_assignable<T, U> is false even
// for a class assignable from anything. Found running GCC's C++
// testsuite.
//@ skip(clang): clang 22 gives the same wrong result
//@ oracle: gcc
//@ expect: compile-ok
struct AssignAll {
  template <class T> AssignAll &operator=(T &&);
};
static_assert(!__is_assignable(AssignAll &, void() const));
static_assert(!__is_assignable(AssignAll &, void() &));
static_assert(__is_assignable(AssignAll &, void()));
