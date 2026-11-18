// [dcl.type.decltype]/1.4, [expr.prim.splice]/2: decltype of an
// unparenthesized splice-expression is the type of the entity, object or
// value its splice-specifier designates, never a reference to it; a
// parenthesized one is an lvalue, so T&. An object no variable names (an
// array element, a template parameter object) is the case that differed:
// kiln gave T&. A class-type reflect_constant result is a template
// parameter object of type const T ([meta.reflection.result]: an object
// for class types; [temp.param]/8), so decltype is const T. Found running
// GCC's C++ testsuite. Not GCC-compared: GCC 16.1 agrees on the element
// and on the parenthesized forms, but drops the const of a template
// parameter object (P and int[2] below) and deduces decltype(auto) from
// a splice as from any lvalue (int&).
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
#include <array>
#include <meta>
#include <type_traits>
using std::is_same_v;

int arr[3];
constexpr int carr[2] = {1, 2};
struct P { int x; };
const P cp{1};
int i;

constexpr auto el = std::meta::reflect_object(arr[1]);
static_assert(is_same_v<decltype([:el:]), int>);
static_assert(is_same_v<decltype(([:el:])), int &>);
constexpr auto cel = std::meta::reflect_object(carr[1]);
static_assert(is_same_v<decltype([:cel:]), const int>);
// A subobject named as a member, a complete object as a variable: their
// declared types.
static_assert(is_same_v<decltype([:std::meta::reflect_object(cp.x):]), int>);
static_assert(is_same_v<decltype([:std::meta::reflect_object(i):]), int>);
static_assert(is_same_v<decltype([:^^arr:]), int[3]>);
static_assert(is_same_v<decltype(([:^^arr:])), int (&)[3]>);
// A value: its type.
static_assert(is_same_v<decltype([:std::meta::reflect_constant(42):]), int>);
// Template parameter objects.
constexpr auto tpo = std::meta::reflect_constant(P{1});
static_assert(is_same_v<decltype([:tpo:]), const P>);
constexpr auto ca = std::meta::reflect_constant_array(std::array{1, 2});
static_assert(is_same_v<decltype([:ca:]), const int[2]>);
static_assert(is_same_v<decltype(([:ca:])), const int (&)[2]>);

// Dependent: the rule applies at substitution, also in a return type.
template <std::meta::info R> using D = decltype([:R:]);
template <std::meta::info R> using DP = decltype(([:R:]));
static_assert(is_same_v<D<el>, int>);
static_assert(is_same_v<DP<el>, int &>);
static_assert(is_same_v<D<tpo>, const P>);
template <std::meta::info R> constexpr auto value_of() -> decltype([:R:]) { return [:R:]; }
static_assert(value_of<cel>() == 2);
static_assert(is_same_v<decltype(value_of<el>()), int>);

// decltype(auto) deduces as decltype does.
decltype(auto) by_value = [:el:];
static_assert(is_same_v<decltype(by_value), int>);
decltype(auto) by_ref = ([:el:]);
static_assert(is_same_v<decltype(by_ref), int &>);

int main() { return value_of<el>(); }
