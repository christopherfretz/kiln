// [basic.lookup.argdep]/3.1 (P2996R13): std::meta is an associated
// namespace of a std::meta::info argument, so an unqualified
// `is_object(r)` finds std::meta::is_object by ADL for an object or value
// reflection, a dependent reflection, a reflection template parameter, a
// reference to a reflection and a reflection data member (WP-562). A
// using-directive for std that makes ordinary lookup find the class
// template std::is_object suppresses ADL instead (/1), which GCC 16.1
// rejects too, so it is not part of this file. Compared with GCC 16.1.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: 1 1 0
#include <meta>
#include <type_traits>
#include <cstdio>

struct L { char t[3]; };
constexpr auto rlit = std::meta::reflect_constant(L{"zz"});
static_assert(is_object(rlit));
int gv;
static_assert(is_object(std::meta::reflect_object(gv)));
static_assert(!is_object(^^int));

template <class T> consteval bool dep() { return is_type(^^T) && !is_object(^^T); }
static_assert(dep<int>());
template <std::meta::info R> consteval bool nttp() { return is_object(R); }
static_assert(nttp<rlit>());
template <class T> consteval bool generic(T r) { return is_object(r); }
static_assert(generic(rlit));
consteval bool byref(const std::meta::info& r) { return is_object(r); }
static_assert(byref(rlit));
struct W { std::meta::info r; };
consteval bool member() { constexpr W w{rlit}; return is_object(w.r); }
static_assert(member());

constexpr bool results[] = {is_object(rlit), nttp<rlit>(), is_object(^^gv)};
int main() { std::printf("%d %d %d\n", results[0], results[1], results[2]); }
