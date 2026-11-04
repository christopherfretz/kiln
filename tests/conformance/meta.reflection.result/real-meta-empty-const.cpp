// [meta.reflection.result] (P3491R3) through libstdc++ 16.1's real <meta>
// (WP-484; spec/reflection.md §3.5), as GCC 16.1 answers: an empty range
// makes reflect_constant_array reflect a `const std::array<T, 0>` object
// (so define_static_array of it is an empty span), and the type of a
// reflect_constant / reflect_constant_array / reflect_constant_string
// result, and of reflect_object of a const object or of one of its
// subobjects, is const-qualified.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
#include <array>
#include <meta>
#include <span>
#include <vector>

constexpr auto e = std::meta::reflect_constant_array(std::span<const int>());
static_assert(std::meta::type_of(e) == ^^const std::array<int, 0>);
static_assert(std::meta::is_object(e) || std::meta::is_variable(e));
static_assert(e == std::meta::reflect_constant_array(std::vector<int>()));
static_assert(e != std::meta::reflect_constant_array(std::vector<long>()));
constexpr auto es = std::define_static_array(std::span<const int>());
static_assert(es.size() == 0 && es.data() == nullptr);
consteval std::size_t empty_vector() {
  std::vector<int> v;
  return std::define_static_array(v).size();
}
static_assert(empty_vector() == 0);

constexpr std::array<int, 2> a{1, 2};
static_assert(std::meta::type_of(std::meta::reflect_constant_array(a)) == ^^const int[2]);
static_assert(std::meta::type_of(std::meta::reflect_constant_string("ab")) == ^^const char[3]);
struct P {
  int x;
};
static_assert(std::meta::type_of(std::meta::reflect_constant(P{1})) == ^^const P);

const int c = 1;
struct S {
  int m;
  const int k;
};
constexpr S s{1, 2};
int mutable_int = 0;
static_assert(std::meta::type_of(std::meta::reflect_object(c)) == ^^const int);
static_assert(std::meta::type_of(std::meta::reflect_object(s)) == ^^const S);
static_assert(std::meta::type_of(std::meta::reflect_object(s.m)) == ^^const int);
static_assert(std::meta::type_of(std::meta::reflect_object(s.k)) == ^^const int);
static_assert(std::meta::type_of(std::meta::reflect_object(mutable_int)) == ^^int);
