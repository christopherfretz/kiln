// [temp.inst]/5 and /8, [meta.reflection.substitute], [meta.reflection.extract]
// (P2996R13; spec/reflection.md §3 "Reading a variable through a reflection",
// WP-638): a static data member of a class template specialization that
// `substitute` produced is reached only through `static_data_members_of`;
// `extract` reads its value, which instantiates its initializer as a
// spelled `cell<T, I>::value` would. The same for a member whose
// initializer names another member not yet instantiated, and a variable
// template specialization `substitute` produced. GCC 16.1 compiles and
// runs the same file; the exit status must agree.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ exit: 42
#include <meta>

template <class T, int I>
struct cell {
  using type = T;
  static constexpr int value = I;
};

template <int I>
struct two {
  static constexpr int a = I;
  static constexpr int b = a * 2;
};

template <int I>
constexpr int tripled = I * 3;

constexpr std::meta::info kinds[] = {^^char, ^^short, ^^int, ^^long long};

// Shape S5 (bench/reflect/s5.cpp), reduced.
consteval int round_trips(int lo, int hi) {
  int sum = 0;
  for (int i = lo; i < hi; ++i) {
    auto r = std::meta::substitute(^^cell, {kinds[i % 4], std::meta::reflect_constant(i)});
    auto args = std::meta::template_arguments_of(r);
    if (args[0] != kinds[i % 4]) return -1;
    auto statics = std::meta::static_data_members_of(r, std::meta::access_context::unchecked());
    sum += std::meta::extract<int>(args[1]) + std::meta::extract<int>(statics[0]);
  }
  return sum;
}
static_assert(round_trips(0, 10) == 90);
static_assert(round_trips(10, 20) == 290);

// `b`'s initializer reads `a`, whose own initializer is instantiated by
// that read.
consteval int chained(int i) {
  auto r = std::meta::substitute(^^two, {std::meta::reflect_constant(i)});
  auto s = std::meta::static_data_members_of(r, std::meta::access_context::unchecked());
  return std::meta::extract<int>(s[1]);
}
static_assert(chained(4) == 8);
static_assert(chained(-3) == -6);

consteval int var_template(int i) {
  return std::meta::extract<int>(std::meta::substitute(^^tripled, {std::meta::reflect_constant(i)}));
}
static_assert(var_template(5) == 15);

constexpr int total = round_trips(0, 3) + chained(6) + var_template(9);  // 6 + 12 + 27

int main() { return total - 3; }
