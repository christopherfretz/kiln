// [expr.mptr.oper]/6 (C++20, P0704R1): a pointer to a `const&`-qualified
// member function may be applied to an rvalue. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
struct S {
  constexpr int get() const& { return 7; }
  constexpr int only_rv() && { return 8; }
};
constexpr int (S::*pg)() const& = &S::get;
constexpr int (S::*pr)() && = &S::only_rv;
static_assert((S{}.*pg)() == 7 && (S{}.*pr)() == 8);
