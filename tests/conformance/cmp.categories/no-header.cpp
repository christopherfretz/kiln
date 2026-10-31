// [expr.spaceship]/8, [class.spaceship]/1: the comparison category types
// are looked up in namespace std; without <compare> a builtin `<=>` and a
// deduced defaulted `operator<=>` are ill-formed.
// The defaulted case has no column: clang points at the declared return
// type, whose location kiln does not record (spec/sema-class.md §11).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:12 error "cannot use builtin operator '<=>' because type 'std::strong_ordering' was not found; include <compare>"
//@ diag: 12:14 error "cannot use builtin operator '<=>' because type 'std::partial_ordering' was not found; include <compare>"
//@ diag: 15 error "cannot default 'operator<=>' because type 'std::strong_ordering' was not found; include <compare>"
auto a = 1 <=> 2;
auto b = 1.0 <=> 2.0;
struct S {
  int x;
  auto operator<=>(const S&) const = default;
};
