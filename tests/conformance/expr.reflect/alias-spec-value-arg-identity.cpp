// [expr.reflect]/6, [temp.alias]/2, [temp.type]/1 P2996R13: `^^Arr<char, 3>`
// reflects the alias template specialization, and two template-ids
// naming it with the same template arguments (template argument 3 however
// written: `3`, `1 + 2`, `(short)3`, `3u`; a defaulted argument; an
// enumerator or `bool` value) are the same specialization, so the
// reflections compare equal. The alias's identity is its converted
// argument list, not the written expressions; the reflection still differs
// from its aliased type, and `template_arguments_of` gives the converted
// values (spec/sema-templates.md §4.10, WP-1168).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
template <class T, int N> using Arr = T[N];
constexpr std::meta::info r = ^^Arr<char, 3>;
static_assert(r == (^^Arr<char, 3>));
static_assert((^^Arr<char, 1 + 2>) == (^^Arr<char, 3>));
static_assert((^^Arr<char, (short)3>) == (^^Arr<char, 3>));
static_assert((^^Arr<char, 3u>) == (^^Arr<char, 1 + 2>));
static_assert((^^Arr<char, 4>) != (^^Arr<char, 3>));
static_assert((^^Arr<char, 3>) != (^^char[3]));

template <class T, int N = 3> using D = T[N];
static_assert((^^D<char>) == (^^D<char, 3>));
static_assert((^^D<char>) != (^^Arr<char, 3>));

enum class E { a, b };
template <E e, bool B> using EB = int[int(e) + 2 * B + 1];
static_assert((^^EB<E::b, true>) == (^^EB<static_cast<E>(1), 1>));
static_assert((^^EB<E::b, true>) != (^^EB<E::a, true>));
static_assert((^^EB<E::a, false>) == (^^EB<E{}, 0 == 1>));

consteval bool args_ok() {
  auto a = std::meta::template_arguments_of(^^Arr<char, 1 + 2>);
  auto d = std::meta::template_arguments_of(^^D<char>);
  return a.size() == 2 && a[0] == (^^char) && std::meta::extract<int>(a[1]) == 3 &&
         d.size() == 2 && std::meta::extract<int>(d[1]) == 3 &&
         std::meta::template_of(^^Arr<char, 1 + 2>) == ^^Arr;
}
static_assert(args_ok());
