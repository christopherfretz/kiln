// [basic.def.odr]/4, [temp.inst]/5, [meta.reflection.extract] (P2996R13;
// spec/reflection.md §3 "Using a function through a reflection",
// WP-1244): a pointer to member function or function pointer that
// `extract` forms to a member of a class template specialization, or to
// a function template specialization, that no expression names is a
// definition the program can call at run time (constexpr or not). GCC
// 16.1 compiles and runs the same file; the output must agree. A splice
// in a call or `&[: r :]` names the function and takes the ordinary path.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: 3 103 31 44 9
//@ stdout: 207 14 8
#include <meta>
#include <cstdio>

template <int N> struct Panel {
  int cells[4] = {N, N + 1, N + 2, N + 3};
  constexpr int at(int i) const { return cells[i]; }
  int add(int i) const { return cells[0] + i; }
  static int scale(int i) { return N * i; }
};

template <int N> constexpr int twice(int i) { return N * i + N; }
template <int N> int plain(int i) { return N + i; }

consteval std::meta::info fn(std::meta::info c, std::string_view name) {
  for (auto m : std::meta::members_of(c, std::meta::access_context::unchecked()))
    if (std::meta::is_function(m) && std::meta::has_identifier(m) && std::meta::identifier_of(m) == name)
      return m;
  return {};
}

constexpr auto at = std::meta::extract<int (Panel<1>::*)(int) const>(fn(^^Panel<1>, "at"));
constexpr auto add = std::meta::extract<int (Panel<100>::*)(int) const>(fn(^^Panel<100>, "add"));
constexpr auto scale = std::meta::extract<int (*)(int)>(fn(^^Panel<31>, "scale"));
constexpr auto tw = std::meta::extract<int (*)(int)>(^^twice<4>);
constexpr auto pl = std::meta::extract<int (*)(int)>(^^plain<5>);
constexpr auto spliced = &[:fn(^^Panel<200>, "add"):];

int main() {
  Panel<1> p1;
  Panel<100> p100;
  std::printf("%d %d %d %d %d\n", (p1.*at)(2), (p100.*add)(3), scale(1), tw(10), pl(4));
  Panel<200> p200;
  std::printf("%d %d %d\n", (p200.*spliced)(7), [:fn(^^Panel<7>, "scale"):](2), p1.[:fn(^^Panel<1>, "at"):](3) * 2);
}
