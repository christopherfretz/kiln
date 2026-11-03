// [dcl.fct.def.default]/3, [class.compare.default]/3 (C++20): a defaulted
// comparison declared constexpr or consteval that would invoke a
// non-constexpr comparison function is an error before C++23, with the
// subobject compared and the first non-constexpr function (none for a
// secondary comparison, which compares the complete object). A defaulted
// <=> also checks its implicit operator== at the <=>, and a class template
// specialization is checked on instantiation.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:27 error "defaulted definition of equality comparison operator cannot be declared constexpr because it invokes a non-constexpr comparison function"
//@ diag: 48:24 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 45:8 note "non-constexpr comparison function declared here"
//@ diag: 49:20 error "defaulted definition of equality comparison operator cannot be declared constexpr because it invokes a non-constexpr comparison function"
//@ diag: 49:15 note "non-constexpr comparison function would be used to compare base class 'NC'"
//@ diag: 45:8 note "non-constexpr comparison function declared here"
//@ diag: 50:23 error "defaulted definition of equality comparison operator cannot be declared constexpr because it invokes a non-constexpr comparison function"
//@ diag: 50:20 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 45:8 note "non-constexpr comparison function declared here"
//@ diag: 51:21 error "defaulted definition of equality comparison operator cannot be declared consteval because it invokes a non-constexpr comparison function"
//@ diag: 51:18 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 45:8 note "non-constexpr comparison function declared here"
//@ diag: 52:22 error "defaulted definition of three-way comparison operator cannot be declared constexpr because it invokes a non-constexpr comparison function"
//@ diag: 52:19 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 46:24 note "non-constexpr comparison function declared here"
//@ diag: 52:22 error "defaulted definition of three-way comparison operator cannot be declared constexpr because for which the corresponding implicit 'operator=='  invokes a non-constexpr comparison function"
//@ diag: 52:19 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 45:8 note "non-constexpr comparison function declared here"
//@ diag: 53:25 error "defaulted definition of three-way comparison operator cannot be declared constexpr because it invokes a non-constexpr comparison function"
//@ diag: 53:22 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 46:24 note "non-constexpr comparison function declared here"
//@ diag: 53:25 error "defaulted definition of three-way comparison operator cannot be declared constexpr because for which the corresponding implicit 'operator=='  invokes a non-constexpr comparison function"
//@ diag: 53:22 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 45:8 note "non-constexpr comparison function declared here"
//@ diag: 57:3 error "defaulted definition of relational comparison operator cannot be declared constexpr because it invokes a non-constexpr comparison function"
//@ diag: 55:24 note "non-constexpr comparison function declared here"
//@ diag: 59:22 error "defaulted definition of equality comparison operator cannot be declared constexpr because it invokes a non-constexpr comparison function"
//@ diag: 59:84 note "non-constexpr comparison function declared here"
//@ diag: 60:38 error "defaulted definition of equality comparison operator cannot be declared constexpr because it invokes a non-constexpr comparison function"
//@ diag: 61:10 note "in instantiation of template class 'Tmpl<NC>' requested here"
//@ diag: 60:35 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 45:8 note "non-constexpr comparison function declared here"
#include <compare>
struct NC {
  bool operator==(const NC&) const { return true; }
  std::strong_ordering operator<=>(const NC&) const { return std::strong_ordering::equal; }
};
struct Mem { int x; NC m; constexpr bool operator==(const Mem&) const = default; };
struct Base : NC { constexpr bool operator==(const Base&) const = default; };
struct Friend { NC m; constexpr friend bool operator==(const Friend&, const Friend&) = default; };
struct Eval { NC m; consteval bool operator==(const Eval&) const = default; };
struct Three { NC m; constexpr std::strong_ordering operator<=>(const Three&) const = default; };
struct Implicit { NC m; constexpr auto operator<=>(const Implicit&) const = default; };
struct Ordered {
  std::strong_ordering operator<=>(const Ordered&) const = default;
  NC m;
  constexpr bool operator<(const Ordered&) const = default;
};
struct NotEq { NC m; constexpr bool operator!=(const NotEq&) const = default; bool operator==(const NotEq&) const = default; };
template<class T> struct Tmpl { T m; constexpr bool operator==(const Tmpl&) const = default; };
Tmpl<NC> t;
bool b = t == t;
struct Fine { int x; constexpr bool operator==(const Fine&) const = default; };
static_assert(Fine{1} == Fine{1});
