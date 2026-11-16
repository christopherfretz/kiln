// [meta.reflection.member.queries]/2-3 (C++26 working draft): a member
// is members-of-eligible only if it is not a specialization of a
// template and its declaration is not a friend declaration; a partial
// specialization is not a template the queries represent. So members_of
// a namespace or a class lists a class template, variable template or
// function template once, never its partial or explicit specializations
// or explicit instantiations, nor friends, using-declarations, the
// injected-class-name or closure types; it lists unnamed bit-fields, an
// anonymous union's class and its unnamed object (a variable at namespace
// scope, a non-static data member in a class). The filters built on the
// list (static_data_members_of, nonstatic_data_members_of) agree, and the
// instantiation of a partial specialization still has template arguments,
// its primary template and typed constructor parameters. GCC 16.1 agrees.
// Reduced from real code (a partial specialization's pattern was listed,
// and type_of its constructor's parameter failed).
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta_queries.h"
#else
#include <meta>
#endif
using namespace std::meta;

constexpr auto all = access_context::unchecked();

// The identifiers of `ms` ("" for a member without one) are `want`.
template <class V>
consteval bool names(const V &ms, std::initializer_list<std::string_view> want) {
  if (ms.size() != want.size()) return false;
  unsigned long i = 0;
  for (std::string_view w : want) {
    info m = ms[i++];
    std::string_view n = has_identifier(m) ? identifier_of(m) : std::string_view("");
    if (n != w) return false;
  }
  return true;
}

namespace ns {
template <class T> struct tup { tup(T) {} };
template <class T> struct tup<T *> { tup(T *) {} };
template <> struct tup<int> { tup(int) {} };
template struct tup<long>;
template <class T> constexpr int vt = 1;
template <class T> constexpr int vt<T *> = 2;
template <> constexpr int vt<int> = 3;
template <class T> void ft(T) {}
template <> void ft<int>(int) {}
template void ft<long>(long);
void other();
struct befriends { friend void other(); friend struct elsewhere; };
using ::ns::other;
inline auto lam = [] {};
static union { int ua; };
struct plain { plain(int) {} };
}  // namespace ns

static_assert(names(members_of(^^ns, all),
                    {"tup", "vt", "ft", "other", "befriends", "lam", "", "", "plain"}));
static_assert(is_class_template(members_of(^^ns, all)[0]));
static_assert(is_variable_template(members_of(^^ns, all)[1]));
static_assert(is_function_template(members_of(^^ns, all)[2]));
static_assert(is_type(members_of(^^ns, all)[6]));
static_assert(is_variable(members_of(^^ns, all)[7]));

struct S;
struct Base { int b; };
struct S : Base {
  int x;
  int : 3;
  union { int ua; float ub; };
  template <class T> struct in {};
  template <class T> struct in<T &> { in(T &) {} };
  template <class T> static constexpr int sv = 0;
  template <class T> static constexpr int sv<T *> = 1;
  static constexpr int sd = 2;
  template <class T> void mf(T) {}
  using Base::b;
  friend struct elsewhere;
  using I = int;
};
template <> struct S::in<int> {};
template <> constexpr int S::sv<int> = 3;
template <> void S::mf<int>(int) {}

// x, the unnamed bit-field, the anonymous union's class and its object,
// in, sv, sd, mf, I, then the six implicit special members.
static_assert(names(members_of(^^S, all),
                    {"x", "", "", "", "in", "sv", "sd", "mf", "I", "", "", "", "", "", ""}));
static_assert(is_class_template(members_of(^^S, all)[4]));
static_assert(is_variable_template(members_of(^^S, all)[5]));
static_assert(names(static_data_members_of(^^S, all), {"sd"}));
static_assert(names(nonstatic_data_members_of(^^S, all), {"x", ""}));
static_assert(bases_of(^^S, all).size() == 1);

// The instantiation of a partial specialization.
static_assert(has_template_arguments(^^ns::tup<int *>));
static_assert(template_of(^^ns::tup<int *>) == ^^ns::tup);
static_assert(template_of(^^ns::tup<int>) == ^^ns::tup);
static_assert(!has_template_arguments(members_of(^^ns, all)[8]));
consteval info ctor_param(info cls) {
  for (info m : members_of(cls, all))
    if (is_constructor(m) && !is_copy_constructor(m) && !is_move_constructor(m))
      return type_of(parameters_of(m)[0]);
  return ^^void;
}
static_assert(ctor_param(^^ns::tup<int *>) == ^^int *);
static_assert(ctor_param(^^ns::tup<int>) == ^^int);
static_assert(ctor_param(^^S::in<long &>) == ^^long &);
