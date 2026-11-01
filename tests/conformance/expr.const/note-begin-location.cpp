// [expr.const]/5, [expr.prim.id.qual], [expr.sub]: a call-site note locates a
// qualified callee at its qualifier (`::k2`, `N::g` also when rebuilt by
// template instantiation) and an out-of-range element step at the begin
// of the subscripted base (`this->b`, `(*this).b`), as clang's getBeginLoc.
// spec/oven.md "Deviations from clang 19".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:38 note "cannot refer to element 3 of array of 1 element in a constant expression"
//@ diag: 36:29 note "in call to 'k(&i)'"
//@ diag: 37:33 note "in call to 'k2(&i)'"
//@ diag: 38:15 note "in call to 'f1()'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:55 note "cannot refer to element 3 of array of 1 element in a constant expression"
//@ diag: 29:45 note "in call to 'g<int>(&i)'"
//@ diag: 39:33 note "in call to 'h<int>(&i)'"
//@ diag: 40:15 note "in call to 'f2()'"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:34 note "cannot refer to element 3 of array of 2 elements in a constant expression"
//@ diag: 41:42 note "in call to 'v.at(3)'"
//@ diag: 42:15 note "in call to 'f3()'"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:35 note "cannot refer to element 3 of array of 2 elements in a constant expression"
//@ diag: 43:42 note "in call to 'v.at2(3)'"
//@ diag: 44:15 note "in call to 'f4()'"
namespace N {
constexpr void k(int *p) { int a[1]; a[*p] = 1; }
template <class T> constexpr void g(T *p) { int a[1]; a[*p] = 1; }
template <class T> constexpr void h(T *p) { N::g(p); }
template <class T> struct V {
  T b[2] = {};
  constexpr T at(int n) { return this->b[n]; }
  constexpr T at2(int n) { return (*this).b[n]; }
};
} // namespace N
constexpr void k2(int *p) { N::k(p); }
constexpr int f1() { int i = 3; ::k2(&i); return 1; }
static_assert(f1());
constexpr int f2() { int i = 3; N::h(&i); return 1; }
static_assert(f2());
constexpr int f3() { N::V<int> v; return v.at(3); }
static_assert(f3());
constexpr int f4() { N::V<int> v; return v.at2(3); }
static_assert(f4());
