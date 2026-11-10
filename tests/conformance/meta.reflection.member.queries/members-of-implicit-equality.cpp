// [meta.reflection.member.queries]/1 (P2996R13), [class.compare.default]/5:
// a class with an explicitly defaulted member operator<=> and no declared
// operator== has an implicitly declared operator== member: the same
// parameter type, ref-qualifier and access, return type bool, defaulted,
// not user-declared or user-provided. members_of lists it after the
// implicit special members (GCC 16.1's order), with a public, private or
// protected <=> alike. A friend <=>'s implicit == is a friend, not a
// member; a user-declared ==, defaulted or not, suppresses the implicit
// one. A class template specialization's implicit == is listed, unless
// the <=>'s trailing requires-clause (which the == shares) is not
// satisfied (WP-1228). Compared with GCC 16.1.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: 1 9 10 8 | 1 8 0 | 1 8 0 | 0 7 | 1 2 1 | 1 2 0 | 1 2 | 1 8 1
#include <meta>
#include <compare>
#include <cstdio>

using namespace std::meta;
constexpr auto all = access_context::unchecked();

struct P { int x = 0, y = 0; auto operator<=>(const P&) const = default; };
class Q { int x; auto operator<=>(const Q&) const = default; };
struct R { int x; protected: auto operator<=>(const R&) const = default; };
struct F { int x; friend auto operator<=>(const F&, const F&) = default; };
struct U { int x; auto operator<=>(const U&) const = default; bool operator==(const U&) const = default; };
struct V { int x; auto operator<=>(const V&) const = default; bool operator==(const V&) const; };
struct X { int x; auto operator<=>(const X&) const = default; bool operator==(int) const; };
struct Y { int x; auto operator<=>(const Y&) const & = default; };

consteval bool is_eq(info m) {
  return is_operator_function(m) && operator_of(m) == operators::op_equals_equals;
}
consteval int n_eq(info c, access_context a = all) {
  int n = 0;
  for (auto m : members_of(c, a)) n += is_eq(m);
  return n;
}
consteval info the_eq(info c) {
  for (auto m : members_of(c, all))
    if (is_eq(m)) return m;
  return ^^int;
}
consteval int idx_eq(info c) {
  int i = 0;
  for (auto m : members_of(c, all)) {
    if (is_eq(m)) return i;
    ++i;
  }
  return -1;
}
consteval int n_fn(info c) {
  int n = 0;
  for (auto m : members_of(c, all)) n += is_function(m);
  return n;
}
consteval int total(info c) { return members_of(c, all).size(); }

template <class T> consteval bool implicit_eq(info c) {
  info e = the_eq(c);
  return is_defaulted(e) && !is_user_declared(e) && !is_user_provided(e) && !is_deleted(e)
      && is_operator_function(e) && is_function(e) && !is_special_member_function(e)
      && !has_identifier(e) && is_class_member(e) && parent_of(e) == c
      && !is_static_member(e) && !is_virtual(e) && is_const(e)
      && (return_type_of(e) == ^^bool) && parameters_of(e).size() == 1
      && type_of(parameters_of(e)[0]) == ^^const T&;
}
static_assert(implicit_eq<P>(^^P) && is_public(the_eq(^^P)) && is_noexcept(the_eq(^^P)));
static_assert(implicit_eq<Q>(^^Q) && is_private(the_eq(^^Q)));
static_assert(implicit_eq<R>(^^R) && is_protected(the_eq(^^R)));
static_assert(implicit_eq<Y>(^^Y) && is_lvalue_reference_qualified(the_eq(^^Y)));
static_assert(!is_lvalue_reference_qualified(the_eq(^^P)));
// A user-declared == is the one listed.
static_assert(is_defaulted(the_eq(^^U)) && is_user_declared(the_eq(^^U)));
static_assert(!is_defaulted(the_eq(^^V)) && is_user_provided(the_eq(^^V)));
static_assert(type_of(parameters_of(the_eq(^^X))[0]) == ^^int);

template <class T> struct T1 { T x; auto operator<=>(const T1&) const = default; };
template <class T> struct T2 {
  T x;
  auto operator<=>(const T2&) const requires (sizeof(T) > 1) = default;
};
struct D : T1<int> { int y; auto operator<=>(const D&) const = default; };
static_assert(n_eq(^^T1<int>) == 1 && total(^^T1<int>) == 9 && implicit_eq<T1<int>>(^^T1<int>));
static_assert(n_eq(^^T2<char>) == 0 && total(^^T2<char>) == 7);
static_assert(n_eq(^^T2<int>) == 1 && total(^^T2<int>) == 9);
static_assert(n_eq(^^D) == 1 && total(^^D) == 9 && implicit_eq<D>(^^D));

int main() {
  constexpr auto pub = access_context::unprivileged();
  std::printf("%d %d %d %d | %d %d %d | %d %d %d | %d %d | %d %d %d | %d %d %d | %d %d | %d %d %d\n",
              n_eq(^^P), idx_eq(^^P), total(^^P), n_fn(^^P),
              n_eq(^^Q), idx_eq(^^Q), n_eq(^^Q, pub),
              n_eq(^^R), idx_eq(^^R), n_eq(^^R, pub),
              n_eq(^^F), total(^^F),
              n_eq(^^U), idx_eq(^^U), n_eq(^^U, pub),
              n_eq(^^V), idx_eq(^^V), int(is_noexcept(the_eq(^^V))),
              n_eq(^^X), idx_eq(^^X),
              n_eq(^^Y), idx_eq(^^Y), n_eq(^^Y, pub));
}
