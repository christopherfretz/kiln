// kiln-internal std::meta surface for the WP-267 reflection spike
// (-freflection; spec/sema.md "Reflection (spike, -freflection)"). Not
// P2996's <meta>: only the names the spike implements. The functions with
// no body are consteval intrinsics the Oven dispatches by qualified name
// (spec/oven.md "Reflection runtime").
#pragma once

namespace std::meta {
using info = decltype(^^int);

consteval info type_of(info r);
consteval const char* name_of(info r);

// members_of: a fixed-capacity array-like (no std::vector in the spike).
consteval unsigned long __kiln_members_count(info cls);
consteval info __kiln_member_at(info cls, unsigned long i);

template <unsigned long N>
struct member_list {
  info items[N == 0 ? 1 : N];
  constexpr unsigned long size() const { return N; }
  constexpr info operator[](unsigned long i) const { return items[i]; }
};
}  // namespace std::meta

// Defines a non-static data member `name` of type `type` in the class
// `cls`, which must be under definition (its member-specification is
// being parsed). Returns the reflection of the new member.
consteval std::meta::info __kiln_define_member(std::meta::info cls, std::meta::info type,
                                               const char* name);
