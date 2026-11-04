// [meta.reflection.names]/8 (P2996R13): source_location_of(r) is
// implementation-defined; kiln gives GCC 16.1's, which keeps one location
// per entity and updates it as the entity is redeclared
// (spec/reflection.md §3.9): a class or enumeration is where it is
// defined (a binding generator derives `#include` lines from it, so a
// forward declaration in a *_fwd.h header must not win); an implicit
// class template specialization is where the primary template's pattern
// is (defined, else declared); a variable or function is where it is
// defined, else where it was last declared; a function template
// specialization is where the template's definition is; a parameter is
// in the definition's declarator, else the first declaration's, at its
// name, or where its decl-specifiers begin when unnamed. A template and
// a namespace stay where they were first declared.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

constexpr unsigned base = __LINE__;
struct Fwd;                          // +1
struct Fwd {                         // +2
  int x;
};
struct OnlyFwd;                      // +5
void fn(int);                        // +6
void fn(int) {}                      // +7
extern int var;                      // +8
int var = 1;                         // +9
enum class E : int;                  // +10
enum class E : int { a };            // +11
void cp(const int a, int b);         // +12
void cp(int a, const int b) {}       // +13
template <class T> struct TT;        // +14
template <class T> struct TT {};     // +15
template <class T> struct P;         // +16
template <class T> struct P<T*> {};  // +17
void decl_only(int x);               // +18
void decl_only(int y);               // +19
void un(int, int b) {}               // +20
template <class T> T ft(T);          // +21
template <class T> T ft(T t) { return t; }  // +22
namespace n1 {}                      // +23
namespace n1 { int q; }              // +24

consteval unsigned line(info r) { return source_location_of(r).line(); }
consteval unsigned col(info r) { return source_location_of(r).column(); }
static_assert(line(^^Fwd) == base + 2);
static_assert(line(^^OnlyFwd) == base + 5);
static_assert(line(^^fn) == base + 7);
static_assert(line(^^var) == base + 9);
static_assert(line(^^E) == base + 11);
static_assert(line(^^cp) == base + 13);
static_assert(line(parameters_of(^^cp)[0]) == base + 13 && col(parameters_of(^^cp)[0]) == 13);
static_assert(line(^^TT) == base + 14);
static_assert(line(^^TT<int>) == base + 15);
static_assert(line(^^P<int*>) == base + 16);
static_assert(line(^^decl_only) == base + 19);
static_assert(line(parameters_of(^^decl_only)[0]) == base + 18);
static_assert(col(parameters_of(^^un)[0]) == 9 && col(parameters_of(^^un)[1]) == 18);
static_assert(line(^^ft<int>) == base + 22);
static_assert(line(^^n1) == base + 23);
static_assert(line(^^n1::q) == base + 24);

int main() {}
