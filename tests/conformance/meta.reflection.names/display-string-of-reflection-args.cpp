// [meta.reflection.names]/6-7 (P2996R13), [temp.arg.nontype]: a
// reflection-valued template argument, in the specialization's
// display_string_of and in that of the argument, prints as GCC 16.1 prints
// a reflection inside a constant (spec/reflection.md §3.9, WP-1229): `^^`
// and the operand fully qualified (`^^tie::Anchor`), a type as
// display_string_of prints it without ` {aka ...}`, an inline or unnamed
// namespace in the scope (`^^ns::in::Q`, `^^ns::{anonymous}::An`), a
// non-member function with its parameter types (`^^ns::g(int, double)`),
// a member function, variable, member or enumerator by its scoped name,
// a member of a class template specialization through the specialization
// (`^^ns::TT<int>::t`), a template by its name and a variable template
// with its parameters (`^^ns::vt<T>`), a value or object reflection as
// `^^` and its spelling (`^^5`, `^^gv`).
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>
#include <string_view>
using namespace std::meta;

template <auto V> struct W {};
template <auto A> struct derived_from_ {};
namespace tie {
struct Anchor {};
}  // namespace tie
namespace ns {
void nf();
void g(int, double);
template <class T> void ft(T);
struct S {
  int m;
  static int sm;
  void mf();
  static void smf(int);
  using A = int;
  enum E { e1 };
};
using Al = S;
namespace a = tie;
enum class F { x };
template <class T> constexpr int vt = 0;
template <class T> concept C = true;
int nv;
inline namespace in {
struct Q {};
}  // namespace in
namespace {
struct An {};
}  // namespace
template <class T> struct TT {
  T t;
};
namespace inner {
struct X {};
}  // namespace inner
}  // namespace ns
void gf();
int gv;

consteval std::string_view arg0(info r) {
  return display_string_of(template_arguments_of(r)[0]);
}

// The specialization and its argument; the extracted reflection itself.
constexpr info t1 = ^^derived_from_<^^tie::Anchor>;
static_assert(display_string_of(t1) == "derived_from_<^^tie::Anchor>");
static_assert(arg0(t1) == "^^tie::Anchor");
static_assert(display_string_of(extract<info>(template_arguments_of(t1)[0])) == "tie::Anchor");

// Types.
static_assert(arg0(^^W<^^ns::S>) == "^^ns::S");
static_assert(arg0(^^W<^^ns::S::A>) == "^^ns::S::A");
static_assert(arg0(^^W<^^ns::S::E>) == "^^ns::S::E");
static_assert(arg0(^^W<^^ns::Al>) == "^^ns::Al");
static_assert(arg0(^^W<^^ns::Q>) == "^^ns::in::Q");
static_assert(arg0(^^W<^^ns::An>) == "^^ns::{anonymous}::An");
static_assert(arg0(^^W<^^ns::inner::X>) == "^^ns::inner::X");
static_assert(arg0(^^W<^^const ns::S*>) == "^^const ns::S*");
static_assert(arg0(^^W<^^ns::S&>) == "^^ns::S&");
static_assert(arg0(^^W<^^int[2]>) == "^^int [2]");
static_assert(arg0(^^W<^^ns::TT<ns::S>>) == "^^ns::TT<ns::S>");
static_assert(display_string_of(^^W<^^ns::TT<ns::S>>) == "W<^^ns::TT<ns::S> >");

// Namespaces.
static_assert(arg0(^^W<^^ns>) == "^^ns");
static_assert(arg0(^^W<^^ns::inner>) == "^^ns::inner");
static_assert(arg0(^^W<^^ns::a>) == "^^ns::a");
static_assert(arg0(^^W<^^::>) == "^^::");

// Functions, variables, members, enumerators.
static_assert(arg0(^^W<^^ns::nf>) == "^^ns::nf()");
static_assert(arg0(^^W<^^ns::g>) == "^^ns::g(int, double)");
static_assert(arg0(^^W<^^gf>) == "^^gf()");
static_assert(arg0(^^W<^^ns::S::mf>) == "^^ns::S::mf");
static_assert(arg0(^^W<^^ns::S::smf>) == "^^ns::S::smf");
static_assert(arg0(^^W<^^ns::nv>) == "^^ns::nv");
static_assert(arg0(^^W<^^gv>) == "^^gv");
static_assert(arg0(^^W<^^ns::S::m>) == "^^ns::S::m");
static_assert(arg0(^^W<^^ns::S::sm>) == "^^ns::S::sm");
static_assert(arg0(^^W<^^ns::S::e1>) == "^^ns::S::e1");
static_assert(arg0(^^W<^^ns::F::x>) == "^^ns::F::x");
static_assert(arg0(^^W<^^ns::TT<int>::t>) == "^^ns::TT<int>::t");
static_assert(display_string_of(^^W<^^ns::nf>) == "W<^^ns::nf()>");

// Templates.
static_assert(arg0(^^W<^^ns::TT>) == "^^ns::TT");
static_assert(arg0(^^W<^^ns::ft>) == "^^ns::ft");
static_assert(arg0(^^W<^^ns::vt>) == "^^ns::vt<T>");
static_assert(arg0(^^W<^^ns::C>) == "^^ns::C");

// Nested: a reflection of a specialization with a reflection argument.
static_assert(arg0(^^W<^^W<^^ns::S>>) == "^^W<^^ns::S>");
static_assert(display_string_of(^^W<^^W<^^ns::S>>) == "W<^^W<^^ns::S> >");

// Values and objects.
static_assert(arg0(^^W<reflect_constant(5)>) == "^^5");
static_assert(arg0(^^W<reflect_constant('a')>) == "^^'a'");
static_assert(arg0(^^W<reflect_constant(1.0)>) == "^^1.0e+0");
static_assert(arg0(^^W<reflect_object(gv)>) == "^^gv");
static_assert(arg0(^^W<reflect_constant(ns::S{1})>) == "^^ns::S{1}");
static_assert(display_string_of(^^W<reflect_constant(5)>) == "W<^^5>");

int main() {}
