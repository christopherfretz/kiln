// [expr.prim.splice]/2 (P2996R13): a splice-expression designating an
// object is an lvalue designating that object. reflect_object of a
// variable and of its subobjects (an array element, an array member, a
// base class subobject) splices to an lvalue naming the subobject, so
// member access, subscripts and & work and the address is the
// subobject's own. A class-type value has no value reflection:
// reflect_constant of one, and template_arguments_of for a class-type
// constant template argument, reflect the template parameter object
// ([temp.param]/8), which is one object per value. GCC 16.1 accepts the
// same file.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <string_view>
#include <type_traits>

struct Label {
  char text[8]{};
  consteval Label(const char* s) {
    for (int i = 0; s[i]; ++i) text[i] = s[i];
  }
};
template <Label L> struct tagged {};

consteval std::meta::info arg0(std::meta::info r) {
  return std::meta::template_arguments_of(r)[0];
}

// The template parameter object of a class-type template argument.
constexpr auto rarg = arg0(^^tagged<"ab">);
static_assert([:rarg:].text[0] == 'a');
static_assert(std::string_view([:rarg:].text) == "ab");
static_assert(std::is_same_v<decltype(([:rarg:])), const Label&>);

// reflect_constant of a structural class: a template parameter object,
// the same one a template argument of that value names.
constexpr auto rlit = std::meta::reflect_constant(Label{"zz"});
static_assert([:rlit:].text[1] == 'z');
static_assert(std::meta::is_object(rlit));
static_assert(&[:rlit:] == &[:std::meta::reflect_constant(Label{"zz"}):]);
template <Label L> constexpr const Label* where = &L;
static_assert(where<Label{"zz"}> == &[:rlit:]);
static_assert(&[:arg0(^^tagged<"zz">):] == &[:rlit:]);

// reflect_object of a variable.
constexpr Label named{"qq"};
constexpr auto rnamed = std::meta::reflect_object(named);
static_assert([:rnamed:].text[0] == 'q');
static_assert(&[:rnamed:] == &named);
static_assert(std::is_same_v<decltype(([:rnamed:])), const Label&>);

// reflect_object of subobjects.
struct Base { int b; };
struct Point : Base { int x; int y[3]; };
constexpr Point pt{{7}, 1, {2, 3, 4}};
constexpr auto relem = std::meta::reflect_object(pt.y[1]);
static_assert([:relem:] == 3);
static_assert(&[:relem:] == &pt.y[1]);
static_assert(std::is_same_v<decltype(([:relem:])), const int&>);
constexpr auto rarr = std::meta::reflect_object(pt.y);
static_assert([:rarr:][2] == 4);
static_assert(&[:rarr:][0] == &pt.y[0]);
constexpr auto rx = std::meta::reflect_object(pt.x);
static_assert([:rx:] == 1);
constexpr auto rbase = std::meta::reflect_object(static_cast<const Base&>(pt));
static_assert([:rbase:].b == 7);
static_assert(&[:rbase:] == static_cast<const Base*>(&pt));
static_assert(std::is_same_v<decltype(([:rbase:])), const Base&>);

// A subobject of a template parameter object.
constexpr auto rtext = std::meta::reflect_object([:rlit:].text[1]);
static_assert(&[:rtext:] == &[:rlit:].text[1]);

// A mutable object: the splice is a modifiable lvalue.
int counters[4] = {10, 20, 30, 40};
constexpr auto rcount = std::meta::reflect_object(counters[2]);
void bump() { [:rcount:] += 5; }
