// [temp.expl.spec]/7: an explicit specialization must be declared before
// the first use that would cause an implicit instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:20 error "explicit specialization of 'A<int>' after instantiation"
//@ diag: 10:15 note "implicit instantiation first required here"
//@ diag: 14:27 error "explicit specialization of 'w' after instantiation"
//@ diag: 13:15 note "implicit instantiation first required here"
template <class T> struct A { static constexpr int v = 0; };
static_assert(A<int>::v == 0);
template <> struct A<int> { static constexpr int v = 1; };
template <class T> constexpr int w = 0;
static_assert(w<int> == 0);
template <> constexpr int w<int> = 1;
