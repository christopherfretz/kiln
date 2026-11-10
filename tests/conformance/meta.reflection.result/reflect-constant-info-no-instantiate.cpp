// [meta.reflection.result]/2-4, [temp.inst]/4 and /8, [expr.const.defns]/2,
// [basic.def.odr]/3-4 (P2996R13; spec/reflection.md §3 "Using a function
// through a reflection", WP-1252): `reflect_constant` of a `std::meta::info`
// value reflects that value; its result depends on the value alone and no
// expression names the function the value represents, so a member function
// of a class template specialization reached through `members_of` is not
// instantiated by `reflect_constant`, `reflect_constant_array` or
// `define_static_array` (which goes through `reflect_constant_array`). A
// body ill-formed for the specialization stays unreached.
//
// GCC 16.1 disagrees by provenance: it instantiates the body when the
// `info` came from `members_of`, but not for the equal
// `^^Panel<2>::operator[]` (spec/landed/reflection/WP-1252.md), hence the
// xfail(gcc).
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 instantiates a members_of function reflection in reflect_constant (WP-1252)
//@ strict-diags
#include <meta>
#include <vector>

template <int N> struct Panel {
  int cells[4] = {1, 2, 3, 4};
  constexpr int operator[](int i) const {
    static_assert(N == 1, "PANEL_BODY_WAS_INSTANTIATED");
    return cells[i];
  }
};

consteval std::vector<std::meta::info> fns() {
  std::vector<std::meta::info> v;
  for (auto m : std::meta::members_of(^^Panel<2>, std::meta::access_context::unchecked()))
    if (std::meta::is_function(m) && !std::meta::is_special_member_function(m)) {
      (void)std::meta::type_of(m);
      v.push_back(m);
    }
  return v;
}

static_assert(fns().size() == 1);
static_assert(fns()[0] == ^^Panel<2>::operator[]);

// V5: reflect_constant of one element; the result reflects the info value.
consteval bool v5() {
  auto v = fns();
  auto r = std::meta::reflect_constant(v[0]);
  return std::meta::is_value(r) &&
         std::meta::extract<std::meta::info>(r) == ^^Panel<2>::operator[];
}
static_assert(v5());

// V3: reflect_constant_array of the vector.
consteval bool v3() {
  auto a = std::meta::reflect_constant_array(fns());
  return std::meta::extent(std::meta::type_of(a)) == 1;
}
static_assert(v3());

// V4: define_static_array of the vector.
consteval bool v4() {
  auto s = std::define_static_array(fns());
  return s.size() == 1 && s[0] == ^^Panel<2>::operator[];
}
static_assert(v4());

int main() {}
