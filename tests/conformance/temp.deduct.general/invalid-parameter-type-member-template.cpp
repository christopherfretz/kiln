// [temp.deduct.general]/11.10, [temp.inst]/1, [stmt.ranged]/1: a member
// template of a class template keeps its parameter's type as declared
// through the class's instantiation, so `T a[3]` with `T = void` is still a
// deduction failure; once fully instantiated, the parameter is plain `T *`:
// a range-`for` over it is an invalid range (as clang), not the
// array-parameter diagnostic given for the template definition.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:78 error "invalid range expression of type 'int *'; no viable 'begin' function available"
//@ diag: 16:23 note "in instantiation of function template specialization 'R<int>::g<int>' requested here"
template <class U> struct S {
  template <class T> static constexpr int h(T a[3], U) { return 1; }
  template <class T> static constexpr int h(...) { return 2; }
};
template <class U> struct R { template <class T> void g(T a[3]) { for (int x : a) (void)x; } };
template void R<int>::g<int>(int*);
static_assert(S<int>::h<void>(nullptr, 0) == 2);
static_assert(S<int>::h<int>(nullptr, 0) == 1);
