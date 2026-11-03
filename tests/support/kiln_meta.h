// kiln-internal std::meta surface for the WP-267 reflection spike
// (-freflection; spec/reflection.md §1.3). Not P2996's <meta>: only the
// names the spike implements. The functions with
// no body are consteval intrinsics the Oven dispatches by qualified name
// and signature (spec/reflection.md §3).
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

namespace std::meta {
// members_of(R) as a fixed-capacity array-like whose size is a constant.
template <info R>
consteval member_list<__kiln_members_count(R)> members_of() {
  member_list<__kiln_members_count(R)> l{};
  for (unsigned long i = 0; i < __kiln_members_count(R); ++i) l.items[i] = __kiln_member_at(R, i);
  return l;
}

consteval bool name_is(info r, const char* s) {
  const char* n = name_of(r);
  for (; *n && *n == *s; ++n, ++s) {
  }
  return *n == *s;
}
}  // namespace std::meta
