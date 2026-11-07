// [meta.reflection.queries] (P2996R13) type_of, [basic.types.general]/6,
// [dcl.array]/8: a variable declared with an array of unknown bound and
// completed by a later declaration (the out-of-class definition of a
// static data member; a namespace-scope definition after an `extern`
// declaration) has the completed type from that point on. type_of follows
// the latest declaration, whether the reflection comes from `^^` or from
// members_of (both designate the first declaration), and so do size_of
// and the type's display string; before the definition the type is still
// the array of unknown bound. Answers checked against GCC 16.1
// (-freflection). WP-795.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif

namespace probe {
struct StaticProbe {
  static const char k_NAME[];
};
// Before the definition: incomplete.
static_assert(std::meta::type_of(^^StaticProbe::k_NAME) == ^^const char[]);
inline const char StaticProbe::k_NAME[] = "SAFE";
} // namespace probe

consteval std::meta::info member_named_k_NAME() {
  for (auto m : std::meta::members_of(^^probe::StaticProbe,
                                      std::meta::access_context::unchecked()))
    if (std::meta::has_identifier(m) && std::meta::identifier_of(m) == "k_NAME")
      return m;
  return {};
}
static_assert(std::meta::type_of(member_named_k_NAME()) == ^^const char[5]);
static_assert(std::meta::type_of(^^probe::StaticProbe::k_NAME) == ^^const char[5]);
static_assert(std::meta::size_of(^^probe::StaticProbe::k_NAME) == 5);
static_assert(std::meta::display_string_of(
                  std::meta::type_of(^^probe::StaticProbe::k_NAME)) == "const char [5]");

// The namespace-scope analogue.
extern const int ns_array[];
static_assert(std::meta::type_of(^^ns_array) == ^^const int[]);
constexpr int ns_array[] = {1, 2, 3};
static_assert(std::meta::type_of(^^ns_array) == ^^const int[3]);
static_assert(std::meta::size_of(^^ns_array) == 3 * sizeof(int));

int main() {}
