// [meta.reflection.names]/6-7 (P2996R13), [ratio.ratio]: libstdc++ 16.1's
// real <meta> and <ratio> (through --gcc-toolchain): the intmax_t
// arguments of std::ratio<86400, 7> print as their decimal values, so a
// template-id rebuilt from template_of and template_arguments_of spells
// the same type as display_string_of of the type itself
// (spec/reflection.md §3.9, WP-564; binding generators write such
// template-ids into generated source). Compared with GCC 16.1
// (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>
#include <ratio>
#include <string>
#include <string_view>
using namespace std::meta;

consteval std::string spell(info r) {
  // GCC 16.1 prints a template reflection as its declaration
  // (`template<long int _Num, long int _Den> struct std::ratio`), so the
  // template name comes from identifier_of.
  std::string s{"std::"};
  s += identifier_of(template_of(r));
  s += '<';
  bool first = true;
  for (info a : template_arguments_of(r)) {
    if (!first) s += ", ";
    first = false;
    s += display_string_of(a);
  }
  return s + '>';
}

constexpr info day_per_week = ^^std::ratio<86400, 7>;
static_assert(display_string_of(template_arguments_of(day_per_week)[0]) == "86400");
static_assert(display_string_of(template_arguments_of(day_per_week)[1]) == "7");
static_assert(spell(day_per_week) == "std::ratio<86400, 7>");
static_assert(spell(day_per_week) == display_string_of(day_per_week));
static_assert(display_string_of(template_arguments_of(dealias(^^std::milli))[1]) == "1000");
static_assert(display_string_of(reflect_constant(std::milli::num)) == "1");

int main() {}
