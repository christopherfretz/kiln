// [meta.reflection.names], [meta.reflection.queries] (P2996R13):
// identifier_of names a class, data member, static member, variable,
// function and namespace; type_of gives the type of a variable, data
// member or function, and a splice of it names that type. With GCC's
// libstdc++ `<meta>` these are std::meta::identifier_of/type_of; without
// a `<meta>` (kiln today) the kiln intrinsics std::meta::name_of/type_of
// are declared by name (spec/reflection.md §1 item 3).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ stdout: Point x y count global fn Geo Shape
//@ stdout: 111110
//@ stdout: 1.25 1
#include <cstdio>
#if __has_include(<meta>)
#include <meta>
consteval const char* name(std::meta::info r) { return std::meta::identifier_of(r).data(); }
#else
namespace std::meta {
using info = decltype(^^int);
consteval info type_of(info r);
consteval const char* name_of(info r);
}
consteval const char* name(std::meta::info r) { return std::meta::name_of(r); }
#endif
using std::meta::info;
using std::meta::type_of;

struct Point { int x; double y; static long count; };
int global;
char fn(short);
namespace Geo { struct Shape {}; }
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };

constexpr const char* names[] = {name(^^Point), name(^^Point::x), name(^^Point::y),
                                 name(^^Point::count), name(^^global), name(^^fn),
                                 name(^^Geo), name(^^Geo::Shape)};
consteval bool has_type(info r, info t) { return type_of(r) == t; }
constexpr bool types[] = {has_type(^^Point::x, ^^int), has_type(^^Point::y, ^^double),
                          has_type(^^Point::count, ^^long), has_type(^^global, ^^int),
                          has_type(^^fn, ^^char(short)), has_type(^^Point::x, ^^double)};
int main() {
  const char* sep = "";
  for (const char* n : names) std::printf("%s%s", sep, n), sep = " ";
  std::printf("\n");
  for (bool b : types) std::printf("%d", b);
  typename [: type_of(^^Point::y) :] d = 1.25;
  std::printf("\n%.2f %d\n", d, same<typename [: type_of(^^fn) :], char(short)>::value);
}
