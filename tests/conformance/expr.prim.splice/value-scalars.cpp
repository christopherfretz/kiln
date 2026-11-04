// [expr.prim.splice]/2 (P2996R13): a splice-expression designating a
// value is a prvalue of the value's type with that value. Value
// reflections of every scalar kind: reflect_constant of an int, enum,
// bool, char, double, nullptr, std::meta::info, pointer to function,
// pointer to member and pointer to an array element, and the value
// reflections template_arguments_of yields for non-class constant
// template arguments ([meta.reflection.queries]). The splice converts
// like any prvalue, and decltype((splice)) is the value's type. GCC 16.1
// accepts the same file (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <type_traits>

enum class Colour { red = 3, blue = 11 };
enum Plain { one = 1, two = 2 };
int bump(int a) { return a + 1; }
struct Rec { int f; constexpr long g() const { return 7; } };
constexpr int table[3] = {5, 6, 7};

constexpr auto r5 = std::meta::reflect_constant(5);
static_assert([:r5:] == 5);
static_assert(std::is_same_v<decltype(([:r5:])), int>);
constexpr long widened = [:r5:];
static_assert(widened == 5L);

constexpr auto rcol = std::meta::reflect_constant(Colour::blue);
static_assert([:rcol:] == Colour::blue);
static_assert(std::is_same_v<decltype(([:rcol:])), Colour>);
constexpr auto rpl = std::meta::reflect_constant(two);
static_assert([:rpl:] + 1 == 3);

constexpr auto rt = std::meta::reflect_constant(true);
static_assert([:rt:]);
static_assert(std::is_same_v<decltype(([:rt:])), bool>);

constexpr auto rch = std::meta::reflect_constant('k');
static_assert([:rch:] == 'k');
static_assert(std::is_same_v<decltype(([:rch:])), char>);

constexpr auto rdb = std::meta::reflect_constant(0.25);
static_assert([:rdb:] * 4 == 1.0);
static_assert(std::is_same_v<decltype(([:rdb:])), double>);

constexpr auto rnp = std::meta::reflect_constant(nullptr);
static_assert([:rnp:] == nullptr);
static_assert(std::is_same_v<decltype(([:rnp:])), std::nullptr_t>);
constexpr const int* null_int = [:rnp:];
static_assert(null_int == nullptr);

constexpr auto rinfo = std::meta::reflect_constant(^^Rec);
static_assert([:rinfo:] == ^^Rec);
static_assert(std::is_same_v<decltype(([:rinfo:])), std::meta::info>);
static_assert(sizeof(typename[:[:rinfo:]:]) == sizeof(Rec));

constexpr auto rfn = std::meta::reflect_constant(&bump);
static_assert([:rfn:] == &bump);
static_assert(std::is_same_v<decltype(([:rfn:])), int (*)(int)>);

constexpr auto rmp = std::meta::reflect_constant(&Rec::f);
static_assert(Rec{41}.*[:rmp:] == 41);
constexpr auto rmf = std::meta::reflect_constant(&Rec::g);
static_assert((Rec{}.*[:rmf:])() == 7);

constexpr auto relem = std::meta::reflect_constant(&table[1]);
static_assert(*[:relem:] == 6);
static_assert([:relem:] == &table[1]);
static_assert([:relem:][1] == 7);

template <int N> struct Num {};
template <Colour C> struct Col {};
template <bool B> struct Flag {};
template <auto V> struct Any {};
consteval std::meta::info arg0(std::meta::info r) {
  return std::meta::template_arguments_of(r)[0];
}
static_assert([:arg0(^^Num<3>):] == 3);
static_assert([:arg0(^^Col<Colour::red>):] == Colour::red);
static_assert(![:arg0(^^Flag<false>):]);
static_assert([:arg0(^^Any<'z'>):] == 'z');
static_assert([:arg0(^^Any<1.5>):] == 1.5);
static_assert([:arg0(^^Any<nullptr>):] == nullptr);
static_assert([:arg0(^^Any<&bump>):] == &bump);
static_assert(Rec{8}.*[:arg0(^^Any<&Rec::f>):] == 8);
static_assert([:arg0(^^Any<^^int>):] == ^^int);
static_assert(*[:arg0(^^Any<&table[2]>):] == 7);
