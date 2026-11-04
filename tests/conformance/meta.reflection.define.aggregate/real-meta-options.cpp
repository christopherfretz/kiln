// [meta.reflection.define.aggregate] over libstdc++ 16.1's real <meta>:
// data_member_options is the header's own aggregate (an optional<_Name>
// name holding a string or u8string, optional<int> alignment and
// bit_width, bool no_unique_address and the P3394R4 vector<info>
// annotations). Each annotation must have a type and stands for its
// constant (a variable or object reflection is its value); the injected
// member carries them in order, without a source location, and equal
// annotation lists make one description. GCC 16.1 compiles and runs the
// same file; the exit status (sizeof(S)) must agree.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ exit: 16
#include <meta>
#include <string_view>
using namespace std::meta;
constexpr int g = 3;
struct A { int x; };
constexpr A ga{1};
struct E {};
struct S;
consteval {
  define_aggregate(^^S, {
    data_member_spec(^^int, {.name = "a"}),
    data_member_spec(^^char, {.name = u8"b", .alignment = 8}),
    data_member_spec(^^unsigned, {.name = std::string_view("c"), .bit_width = 3}),
    data_member_spec(^^E, {.name = "e", .no_unique_address = true}),
    data_member_spec(^^int, {.name = "n", .annotations = {reflect_object(g), ^^g, reflect_constant(ga), reflect_constant(5), reflect_constant(^^S)}}),
    data_member_spec(^^int, {.bit_width = 0}),
  });
}
consteval info mem(int i) { return nonstatic_data_members_of(^^S, access_context::unchecked())[i]; }
consteval std::size_t members() { return nonstatic_data_members_of(^^S, access_context::unchecked()).size(); }
consteval info an(int i) { return annotations_of(mem(4))[i]; }
consteval std::size_t count(info r) { return annotations_of(r).size(); }
consteval std::size_t count_int(info r) { return annotations_of_with_type(r, ^^int).size(); }
static_assert(members() == 5);
static_assert(identifier_of(mem(1)) == "b");
static_assert(alignment_of(mem(1)) == 8);
static_assert(offset_of(mem(1)).bytes == 8);
static_assert(bit_size_of(mem(2)) == 3 && is_bit_field(mem(2)));
static_assert(count(mem(0)) == 0);
static_assert(count(mem(4)) == 5);
static_assert(is_annotation(an(0)));
static_assert(extract<int>(an(0)) == 3);
static_assert(extract<int>(an(1)) == 3);
static_assert(extract<A>(an(2)).x == 1);
static_assert(extract<int>(an(3)) == 5);
static_assert(extract<info>(an(4)) == ^^S);
static_assert(constant_of(an(0)) == reflect_constant(3));
static_assert(is_value(constant_of(an(0))) && is_object(constant_of(an(2))));
static_assert(an(0) != an(1) && an(0) == an(0));
static_assert(count_int(mem(4)) == 3);
static_assert(source_location_of(an(0)).line() == 0);

// Equal annotation lists are one description.
static_assert(data_member_spec(^^int, {.name = "a", .annotations = {reflect_constant(1)}}) ==
              data_member_spec(^^int, {.name = "a", .annotations = {reflect_constant(1)}}));
static_assert(data_member_spec(^^int, {.name = "a", .annotations = {reflect_constant(1)}}) !=
              data_member_spec(^^int, {.name = "a"}));
static_assert(data_member_spec(^^int, {.name = "a", .annotations = {reflect_constant(1)}}) !=
              data_member_spec(^^int, {.name = "a", .annotations = {reflect_constant(2)}}));

// An annotation without a type, and an invalid name, throw.
consteval bool throws(info r) {
  try {
    (void)data_member_spec(^^int, {.name = "a", .annotations = {r}});
  } catch (const exception& e) {
    return e.from() == ^^data_member_spec &&
           std::string_view(e.what()) == "reflection does not have a type";
  }
  return false;
}
static_assert(throws(^^int) && throws(info{}) && throws(^^std));
consteval bool bad_name() {
  try {
    (void)data_member_spec(^^int, {.name = "1x"});
  } catch (const exception& e) {
    return e.from() == ^^data_member_spec;
  }
  return false;
}
static_assert(bad_name());

int main() {
  S s{};
  s.n = 4;
  s.c = 7;
  return s.n + s.c == 11 ? int(sizeof(S)) : 1;
}
