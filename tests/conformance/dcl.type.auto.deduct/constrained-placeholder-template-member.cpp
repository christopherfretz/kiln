// [dcl.type.auto.deduct]/2: the type deduced for a constrained placeholder
// must satisfy its constraint, checked in each specialization of a class
// template's static data member and of a variable template. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 17:17 error "deduced type 'int' does not satisfy 'Same<long>'"
//@ diag: 20:9 note "in instantiation of template class 'S<long>' requested here"
//@ diag: 15:44 note "because '__is_same(int, long)' evaluated to false"
//@ diag: 18:20 error "deduced type 'int' does not satisfy 'Same<long>'"
//@ diag: 15:44 note "because '__is_same(int, long)' evaluated to false"
//@ diag: 21:20 error "deduced type 'int' does not satisfy 'Same<long>'"
//@ diag: 22:9 note "in instantiation of variable template specialization 'vt' requested here"
//@ diag: 15:44 note "because '__is_same(int, long)' evaluated to false"
template <class T, class U> concept Same = __is_same(T, U);
template <class T> struct S {
  static inline Same<T> auto v = 0;
  static constexpr Same<T> auto w = 0;
};
int x = S<long>::v;
template <class T> Same<T> auto vt = 0;
int z = vt<long>;
// Valid: the constraint is satisfied.
template <class T> struct V {
  static constexpr Same<T> auto n = T(3);
  int a[n];
};
V<int> ok;
static_assert(sizeof(ok.a) == 3 * sizeof(int));
template <class T> constexpr Same<T> auto vt2 = T(1);
static_assert(vt2<long> == 1);
int y = S<int>::v;
