// [meta.reflection.queries] (P3394R4): an annotation has a type
// (has-type), and type_of(a) is the type of its underlying constant,
// i.e. type_of(constant_of(a)) ([dcl.attr.annotation]/2): a class-type
// annotation's constant is a template parameter object, so its type is
// const T (and is_const(a) holds); a scalar annotation's is the
// cv-unqualified T reflect_constant deduces. This holds for a constexpr
// variable of class type, a class prvalue and a literal, on member
// functions, data members, namespace-scope functions, variables and
// classes, and through a consteval scan that strips the const with
// remove_const over members_of(^^C, access_context::unchecked()).
// Answers checked against GCC 16.1 (-freflection).
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: run
//@ oracle: gcc
//@ strict-diags
//@ exit: 0
#include <meta>
using namespace std::meta;

struct skip_t {};
inline constexpr skip_t skip{};
struct verb_t { int id; };

struct [[=skip]] C {
  [[=skip]] void f();
  [[=skip_t{}]] int m;
  [[=1]] int n;
  [[=verb_t{7}, =skip]] void g();
  int plain;
};
[[=1]] void nf();
[[=skip_t{}]] void nf2();
[[=skip]] int v;
[[=1.5f]] int w;
enum E { e1 };
[[=e1]] int w2;
template <class T> struct Tp { [[=T{}]] int x; };

// A constexpr variable of class type: const T.
static_assert(type_of(annotations_of(^^C::f)[0]) == ^^const skip_t);
static_assert(type_of(annotations_of(^^v)[0]) == ^^const skip_t);
static_assert(type_of(annotations_of(^^C)[0]) == ^^const skip_t);
// A class prvalue: const T too.
static_assert(type_of(annotations_of(^^C::m)[0]) == ^^const skip_t);
static_assert(type_of(annotations_of(^^nf2)[0]) == ^^const skip_t);
static_assert(type_of(annotations_of(^^C::g)[0]) == ^^const verb_t);
static_assert(type_of(annotations_of(^^C::g)[1]) == ^^const skip_t);
// A substituted annotation of a class template's member.
static_assert(type_of(annotations_of(^^Tp<skip_t>::x)[0]) == ^^const skip_t);
static_assert(type_of(annotations_of(^^Tp<int>::x)[0]) == ^^int);
// A scalar: the cv-unqualified type.
static_assert(type_of(annotations_of(^^C::n)[0]) == ^^int);
static_assert(type_of(annotations_of(^^nf)[0]) == ^^int);
static_assert(type_of(annotations_of(^^w)[0]) == ^^float);
static_assert(type_of(annotations_of(^^w2)[0]) == ^^E);
// type_of(a) == type_of(constant_of(a)).
static_assert(type_of(annotations_of(^^C)[0]) ==
              type_of(constant_of(annotations_of(^^C)[0])));
static_assert(type_of(annotations_of(^^nf)[0]) ==
              type_of(constant_of(annotations_of(^^nf)[0])));
// is_const / is_volatile inspect type_of(a).
static_assert(is_const(annotations_of(^^C)[0]));
static_assert(is_const(annotations_of(^^C::m)[0]));
static_assert(!is_const(annotations_of(^^nf)[0]));
static_assert(!is_volatile(annotations_of(^^C)[0]));

// The annotation scanner: every annotation's type, const stripped.
consteval bool has_tag(info r, info tag) {
  for (info a : annotations_of(r))
    if (remove_const(type_of(a)) == tag) return true;
  return false;
}
consteval int count_tagged(info cls, info tag) {
  int k = has_tag(cls, tag) ? 100 : 0;
  for (info m : members_of(cls, access_context::unchecked()))
    if (has_tag(m, tag)) ++k;
  return k;
}
static_assert(count_tagged(^^C, ^^skip_t) == 103);
static_assert(count_tagged(^^C, ^^verb_t) == 1);
static_assert(count_tagged(^^C, ^^int) == 1);

int main() {
  // The same scan at run time through a constant.
  constexpr int k = count_tagged(^^C, ^^skip_t);
  return k == 103 ? 0 : 1;
}
