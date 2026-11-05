// [temp.inst]/5 and /8, [meta.reflection.extract], [meta.reflection.queries]
// (P2996R13; spec/reflection.md §3 "Reading a variable through a reflection",
// WP-638): reading a static data member of a spelled specialization that
// no expression names, through `static_data_members_of` or `members_of`,
// instantiates its initializer: `extract`, `constant_of`, `object_of` and
// a splice. `type_of` and `is_const` only need its declaration, so a
// member whose initializer would be ill-formed is still reflectable
// ([temp.inst]/3: the initializer is not instantiated). GCC 16.1
// compiles and runs the same file; the exit status must agree.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ exit: 21
#include <meta>

template <class T, int I>
struct cell {
  static constexpr int value = I;
};

template <class T>
struct bad {
  static constexpr int x = T::nope;
};

consteval std::meta::info first_static(std::meta::info r) {
  return std::meta::static_data_members_of(r, std::meta::access_context::unchecked())[0];
}

static_assert(std::meta::extract<int>(first_static(^^cell<long, 9>)) == 9);
static_assert(std::meta::extract<int>(std::meta::constant_of(first_static(^^cell<char, 12>))) == 12);
static_assert(std::meta::extract<const int&>(std::meta::object_of(first_static(^^cell<unsigned, 13>))) == 13);

consteval int via_members_of() {
  for (auto m : std::meta::members_of(^^cell<short, 14>, std::meta::access_context::unchecked())) {
    if (std::meta::is_variable(m)) return std::meta::extract<int>(m);
  }
  return -1;
}
static_assert(via_members_of() == 14);

constexpr std::meta::info spliced = first_static(^^cell<double, 11>);
static_assert([:spliced:] == 11);

constexpr std::meta::info unchecked = first_static(^^bad<int>);
static_assert(std::meta::remove_const(std::meta::type_of(unchecked)) == ^^int);
static_assert(std::meta::is_const(unchecked));
static_assert(std::meta::identifier_of(unchecked) == "x");

constexpr int runtime = std::meta::extract<int>(first_static(^^cell<bool, 21>));

int main() { return runtime; }
