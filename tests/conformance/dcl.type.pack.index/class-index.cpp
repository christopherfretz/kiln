// [expr.prim.pack.index]/1, [dcl.type.pack.index]/1: the index is a
// converted constant expression of type std::size_t, so an index of class
// type goes through a constexpr conversion function (in an expression and
// in a type); a non-constexpr one is not a constant, and a class with no
// conversion is not convertible. Found running GCC's C++ testsuite.
//@ requires: clang>=22
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ diag: 29:63 error "array size is not a constant expression"
//@ diag: 29:63 note "non-constexpr function 'operator int' cannot be used in a constant expression"
//@ diag: 27:3 note "declared here"
//@ diag: 32:64 error "value of type 'N' is not implicitly convertible to '__size_t' (aka 'unsigned long')"
struct A {
  constexpr operator int() const { return 1; }
};
template <class... T> constexpr int pick(T... t) {
  constexpr A a;
  return t...[a];
}
static_assert(pick(1, 2, 3) == 2);
template <class... T> using Second = T...[A{}];
static_assert(sizeof(Second<char, int>) == sizeof(int));

struct E {
  operator int() const { return 0; }
};
template <class... T> constexpr int bad(T... t) { return t...[E{}]; }
int x = bad(1, 2);
struct N {};
template <class... T> constexpr int none(T... t) { return t...[N{}]; }
int y = none(1, 2);
