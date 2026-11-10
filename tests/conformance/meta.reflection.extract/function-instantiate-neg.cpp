// [basic.def.odr]/4, [temp.inst]/5 and /8, [meta.reflection.extract]
// (P2996R13; spec/reflection.md §3 "Using a function through a
// reflection", WP-1244): `extract` of a function to a pointer to member
// function or a function pointer odr-uses it, so the definition of a
// member of a class template specialization (or of a function template
// specialization) is instantiated, at once for a constexpr function; one
// ill-formed for the specialization is diagnosed there, with the
// instantiation's note at the `extract` call (as GCC 16.1).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 32:19 error "static assertion failed due to requirement '2 == 1': MEMBER_BODY"
//@ diag: 47:13 note "in instantiation of member function 'Panel<2>::operator[]' requested here"
//@ diag: 36:19 error "static assertion failed due to requirement '3 == 1': STATIC_BODY"
//@ diag: 53:21 note "in instantiation of member function 'Panel<3>::make' requested here"
//@ diag: 41:63 error "static assertion failed due to requirement '4 == 1': FREE_BODY"
//@ diag: 54:21 note "in instantiation of function template specialization 'free_fn<4>' requested here"
#include <meta>

consteval std::meta::info nth_function(std::meta::info c, int k) {
  for (auto m : std::meta::members_of(c, std::meta::access_context::unchecked()))
    if (std::meta::is_function(m) && !std::meta::is_special_member_function(m) && k-- == 0)
      return m;
  return {};
}

template <int N> struct Panel {
  int cells[4] = {1, 2, 3, 4};
  constexpr int operator[](int i) const {
    static_assert(N == 1, "MEMBER_BODY");
    return cells[i];
  }
  static constexpr int make(int i) {
    static_assert(N == 1, "STATIC_BODY");
    return i;
  }
};

template <int N> constexpr int free_fn(int i) { static_assert(N == 1, "FREE_BODY"); return i; }

consteval int walk() {
  int n = 0;
  for (auto m : std::meta::members_of(^^Panel<2>, std::meta::access_context::unchecked()))
    if (std::meta::is_function(m) && !std::meta::has_identifier(m) && !std::meta::is_special_member_function(m)) {
      (void)std::meta::extract<int (Panel<2>::*)(int) const>(m);
      ++n;
    }
  return n;
}
static_assert(walk() == 1);
constexpr auto pf = std::meta::extract<int (*)(int)>(nth_function(^^Panel<3>, 1));
constexpr auto pg = std::meta::extract<int (*)(int)>(^^free_fn<4>);

int main() {}
