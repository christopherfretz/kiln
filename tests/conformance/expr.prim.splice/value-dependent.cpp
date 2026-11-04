// [expr.prim.splice]/2, [temp.dep.splice] (P2996R13): a splice of a
// value-dependent reflection is resolved when the template is
// instantiated, to the same prvalue (value reflection) or lvalue (object
// reflection) a non-dependent splice gives. Function templates, consteval
// function templates reading the literal a class-type template argument
// carries (through dealias of an alias), variable templates and class
// template members. Pointer and reference template arguments that point
// into a subobject stand for the subobject ([temp.arg.nontype]). GCC 16.1
// accepts the same file.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <string_view>

struct Word {
  char chars[8]{};
  consteval Word(const char* s) {
    for (int i = 0; s[i]; ++i) chars[i] = s[i];
  }
};
template <Word W> struct spelled {};
template <class T> using same_t = T;
template <int N> struct count {};

template <std::meta::info P> consteval char second_char() {
  constexpr auto arg = std::meta::template_arguments_of(std::meta::dealias(P))[0];
  constexpr std::string_view sv([:arg:].chars);
  return sv[1];
}
static_assert(second_char<^^spelled<"ab">>() == 'b');
static_assert(second_char<^^same_t<spelled<"xy">>>() == 'y');

template <std::meta::info P> consteval int count_of() {
  constexpr auto arg = std::meta::template_arguments_of(P)[0];
  return [:arg:] * 10;
}
static_assert(count_of<^^count<4>>() == 40);

template <std::meta::info R> constexpr auto spliced = [:R:];
static_assert(spliced<std::meta::reflect_constant(42)> == 42);
static_assert(spliced<std::meta::reflect_constant('c')> == 'c');

template <std::meta::info R> constexpr char first_of = [:R:].chars[0];
static_assert(first_of<std::meta::reflect_constant(Word{"mn"})> == 'm');

constexpr int cells[3] = {4, 5, 6};
template <std::meta::info R> constexpr const int* address_of = &[:R:];
static_assert(address_of<std::meta::reflect_object(cells[1])> == &cells[1]);

template <std::meta::info R> struct Box {
  static constexpr auto value = [:R:];
  constexpr auto get() const { return [:R:] + 1; }
};
static_assert(Box<std::meta::reflect_constant(9)>::value == 9);
static_assert(Box<std::meta::reflect_constant(9)>{}.get() == 10);

template <auto V> constexpr auto through_reflection() {
  return [:std::meta::reflect_constant(V):];
}
static_assert(through_reflection<17>() == 17);

// Pointer and reference template arguments designating subobjects.
struct Inner { int a; int b[2]; };
struct Outer { int head; Inner in; };
constexpr Outer outer{1, {2, {3, 4}}};
template <const int* Q> constexpr int deref = *Q;
static_assert(deref<&outer.in.b[1]> == 4);
template <const int& R> constexpr int read() { return R; }
static_assert(read<outer.in.a>() == 2);
template <const Inner& R> constexpr int read_inner() { return R.b[0]; }
static_assert(read_inner<outer.in>() == 3);
template <const int& R> struct Ref {};
consteval std::meta::info arg0(std::meta::info r) {
  return std::meta::template_arguments_of(r)[0];
}
constexpr auto rref = arg0(^^Ref<outer.in.b[0]>);
static_assert(std::meta::is_object(rref));
static_assert([:rref:] == 3);
static_assert(&[:rref:] == &outer.in.b[0]);
