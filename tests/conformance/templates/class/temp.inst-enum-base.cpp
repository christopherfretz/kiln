// [dcl.enum]/2, [temp.res.general]/6: a dependent enum-base is checked at
// instantiation (not in the template definition).
//@ expect: compile-ok
template <class T> struct E { enum X : T {}; };
E<int> fine;
static_assert(sizeof(E<int>::X) == sizeof(int));
