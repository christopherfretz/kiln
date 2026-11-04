// The header-differential stand-in for tests/support/kiln_meta.h
// (`cargo xtask test --meta-header`; spec/harness.md "Header-differential
// mode"). The mode puts this directory first on the include path, so a
// reflection test's `#include "kiln_meta.h"` reads libstdc++'s real
// <meta> (GCC 16.1's, through --gcc-toolchain) instead of kiln's
// declarations, plus the test helpers kiln_meta.h defines that are not
// in the standard, rebuilt here over the standard API. kiln extensions
// (`__kiln_define_member`, `__kiln_reflection_range`, ...) are not
// declared: the mode skips the tests that use them.
#pragma once

#include <exception>
#include <initializer_list>
#include <meta>
#include <optional>
#include <source_location>
#include <span>
#include <string_view>
#include <vector>

namespace std::meta {
// kiln_meta.h's array-like of nonstatic_data_members_of(R), for expansion
// statements.
template <unsigned long N>
struct __kiln_info_array {
  info items[N == 0 ? 1 : N];
  constexpr unsigned long size() const { return N; }
  constexpr info operator[](unsigned long i) const { return items[i]; }
};
template <info R>
consteval auto __kiln_data_members() {
  constexpr unsigned long n = nonstatic_data_members_of(R, access_context::unchecked()).size();
  __kiln_info_array<n> l{};
  unsigned long i = 0;
  for (info m : nonstatic_data_members_of(R, access_context::unchecked())) l.items[i++] = m;
  return l;
}

// kiln_meta.h's name_is (over the kiln extension name_of): the entity's
// identifier, or the empty string when it has none.
consteval bool name_is(info r, const char* s) {
  return (has_identifier(r) ? identifier_of(r) : std::string_view()) == std::string_view(s);
}
}  // namespace std::meta
