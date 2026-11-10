// [temp.inst]/3 and /5, [meta.reflection.queries] (P2996R13;
// spec/reflection.md §3 "Using a function through a reflection",
// WP-1244): declaration queries of a member function of a class template
// specialization (`members_of`, `type_of`, `is_function`, ...) only need
// its declaration, so a body that would be ill-formed for the
// specialization is not instantiated; nor is one only reached by a
// failed `extract` (a type mismatch). GCC 16.1 accepts the same file.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>

template <int N> struct Panel {
  int cells[4] = {1, 2, 3, 4};
  constexpr int operator[](int i) const {
    static_assert(N == 1, "PANEL_BODY_WAS_INSTANTIATED");
    return cells[i];
  }
  static constexpr int make(int i) {
    static_assert(N == 1, "STATIC_BODY_WAS_INSTANTIATED");
    return i;
  }
};

consteval int walk() {
  int n = 0;
  for (auto m : std::meta::members_of(^^Panel<2>, std::meta::access_context::unchecked()))
    if (std::meta::is_function(m) && !std::meta::is_special_member_function(m)) {
      (void)std::meta::type_of(m);
      (void)std::meta::is_static_member(m);
      (void)std::meta::parameters_of(m);
      ++n;
    }
  return n;
}
static_assert(walk() == 2);

// `Panel<1>` is fine: extracting its members instantiates them.
consteval int sum() {
  int n = 0;
  for (auto m : std::meta::members_of(^^Panel<1>, std::meta::access_context::unchecked()))
    if (std::meta::is_function(m) && !std::meta::is_special_member_function(m)) {
      if (std::meta::is_static_member(m))
        n += std::meta::extract<int (*)(int)>(m)(10);
      else
        n += (Panel<1>{}.*std::meta::extract<int (Panel<1>::*)(int) const>(m))(2);
    }
  return n;
}
static_assert(sum() == 13);

int main() {}
