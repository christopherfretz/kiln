// [meta.reflection.names]/6-7 (P2996R13): display_string_of is
// implementation-defined; kiln answers GCC 16.1's string (spec/reflection.md
// §3.9), which formats a type as GCC's diagnostic `%T` and a declaration as
// `%D` (gcc/cp/reflect.cc eval_display_string_of): GCC's names of the
// fundamental types (`long unsigned int`), declarator spacing (`const
// char*`, `int [3]`, `void(int, ...)`, `int (* (*)(int))[3]`), `> >`
// between closers, a type alias with its `{aka ...}`, inline and unnamed
// namespaces in the scope, and a function with its specifiers, return type
// and qualifiers. A binding generator spells the types it emits with
// display_string_of, so these strings reach generated code.
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

namespace ns {
struct S {
  int m;
  static int sm;
  void f();
  void g(int, double) const;
  static int sf();
  virtual void v();
  int h() &&;
  void k() const &;
  constexpr int ce() const { return 1; }
  S& operator=(const S&);
  bool operator==(const S&) const;
  void* operator new(decltype(sizeof 0));
  operator int() const;
  struct Inner {
    enum In { x };
  };
  enum class F2 { q };
};
enum E { e1, e2 };
enum class F : unsigned char { a, b };
template <class T, int N = 3> struct TT {};
template <class... Ts> struct V {};
template <template <class> class X> struct TTP {};
template <class T> struct Q {};
using Al = S;
typedef int I;
typedef const int CI;
using FP = void (*)(int);
inline namespace inl {
struct InS {};
}  // namespace inl
namespace {
struct Anon {};
}  // namespace
int gv;
void free_fn(int, ...);
union U {
  int a;
};
template <class T> struct Holder {};
template <class T, class U = T> struct I2 {};
// First formed with its default written out: GCC prints the arguments
// written where a specialization is first formed.
using Expl = I2<char, char>;
// First formed by substitution: GCC counts every argument.
template <class T> struct User {
  using X = I2<T>;
};
using UX = User<long>::X;
struct D : S, private TT<int> {};
}  // namespace ns
template <class T> struct G {};

consteval bool eq(std::string_view a, const char* b) {
  decltype(sizeof 0) n = 0;
  while (b[n]) {
    if (n >= a.size() || a.data()[n] != b[n]) return false;
    ++n;
  }
  return n == a.size();
}
#define CHECK(r, s) static_assert(eq(display_string_of(r), s))

// Fundamental types: GCC's names.
CHECK(^^int, "int");
CHECK(^^unsigned, "unsigned int");
CHECK(^^long, "long int");
CHECK(^^unsigned long, "long unsigned int");
CHECK(^^long long, "long long int");
CHECK(^^unsigned long long, "long long unsigned int");
CHECK(^^short, "short int");
CHECK(^^unsigned short, "short unsigned int");
CHECK(^^signed char, "signed char");
CHECK(^^bool, "bool");
CHECK(^^long double, "long double");
CHECK(^^decltype(nullptr), "std::nullptr_t");
// Declarators.
CHECK(^^const int, "const int");
CHECK(^^const char*, "const char*");
CHECK(^^int&, "int&");
CHECK(^^int&&, "int&&");
CHECK(^^const volatile int*, "const volatile int*");
CHECK(^^const int* const, "const int* const");
CHECK(^^ns::S* const*, "ns::S* const*");
CHECK(^^int (*)(int), "int (*)(int)");
CHECK(^^int[3], "int [3]");
CHECK(^^int[], "int []");
CHECK(^^const char[4], "const char [4]");
CHECK(^^char*[2], "char* [2]");
CHECK(^^int (&)[2], "int (&)[2]");
CHECK(^^int (&&)[2], "int (&&)[2]");
CHECK(^^void (&)(int), "void (&)(int)");
CHECK(^^void(int, ...), "void(int, ...)");
CHECK(^^void() noexcept, "void() noexcept");
CHECK(^^int (*(*)(int))[3], "int (* (*)(int))[3]");
CHECK(^^int ns::S::*, "int ns::S::*");
CHECK(^^void (ns::S::*)(), "void (ns::S::*)()");
CHECK(^^int (ns::S::*)(int) const, "int (ns::S::*)(int) const");
// Classes, enumerations, scopes and template arguments.
CHECK(^^ns::S, "ns::S");
CHECK(^^ns::S::Inner, "ns::S::Inner");
CHECK(^^ns::S::Inner::In, "ns::S::Inner::In");
CHECK(^^ns::E, "ns::E");
CHECK(^^ns::U, "ns::U");
CHECK(^^ns::InS, "ns::inl::InS");
CHECK(^^ns::Anon, "ns::{anonymous}::Anon");
CHECK(^^ns::TT<int>, "ns::TT<int>");
CHECK((^^ns::TT<int, 4>), "ns::TT<int, 4>");
CHECK(^^ns::TT<ns::TT<int>>, "ns::TT<ns::TT<int> >");
CHECK(^^ns::V<>, "ns::V<>");
CHECK((^^ns::V<int, char>), "ns::V<int, char>");
CHECK(^^ns::TTP<ns::Q>, "ns::TTP<ns::Q>");
CHECK(^^G<const ns::S*>, "G<const ns::S*>");
CHECK(^^ns::I2<char>, "ns::I2<char, char>");
CHECK(^^ns::I2<int>, "ns::I2<int>");
CHECK(^^ns::I2<long>, "ns::I2<long int, long int>");
CHECK(^^ns::UX, "ns::UX {aka ns::I2<long int, long int>}");
// Aliases print with what they alias.
CHECK(^^ns::Al, "ns::Al {aka ns::S}");
CHECK(^^ns::I, "ns::I {aka int}");
CHECK(^^ns::CI, "ns::CI {aka const int}");
CHECK(^^const ns::I, "const int");
CHECK(^^ns::FP, "ns::FP {aka void (*)(int)}");
// Declarations.
CHECK(^^ns, "ns");
CHECK(^^::, "::");
CHECK(^^ns::gv, "ns::gv");
CHECK(^^ns::S::m, "ns::S::m");
CHECK(^^ns::S::sm, "ns::S::sm");
CHECK(^^ns::e2, "ns::e2");
CHECK(^^ns::F::b, "ns::F::b");
CHECK(^^ns::S::Inner::x, "ns::S::Inner::x");
CHECK(^^ns::S::F2::q, "ns::S::F2::q");
CHECK(^^ns::S::f, "void ns::S::f()");
CHECK(^^ns::S::g, "void ns::S::g(int, double) const");
CHECK(^^ns::S::sf, "static int ns::S::sf()");
CHECK(^^ns::S::v, "virtual void ns::S::v()");
CHECK(^^ns::S::h, "int ns::S::h() &&");
CHECK(^^ns::S::k, "void ns::S::k() const &");
CHECK(^^ns::S::ce, "constexpr int ns::S::ce() const");
CHECK(^^ns::S::operator=, "ns::S& ns::S::operator=(const ns::S&)");
CHECK(^^ns::S::operator==, "bool ns::S::operator==(const ns::S&) const");
CHECK(^^ns::S::operator int, "ns::S::operator int() const");
CHECK(^^ns::free_fn, "void ns::free_fn(int, ...)");
CHECK(^^ns::TT, "template<class T, int N> struct ns::TT");
CHECK(^^ns::V, "template<class ... Ts> struct ns::V");
CHECK(bases_of(^^ns::D, access_context::unchecked())[1], "ns::D: ns::TT<int>");
CHECK(parameters_of(^^ns::S::g)[0], "<unnamed parameter 1 of void ns::S::g(int, double) const>");

int main() {}
