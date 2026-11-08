// [expr.prim.splice]/2 (P2996R13): a splice of a reflection of an object
// is an lvalue designating that object, including a static-storage object
// no declaration names: a string literal object ([lex.string]/9), the
// `std::type_info` object of a typeid ([expr.typeid]/1) and a
// lifetime-extended temporary ([class.temporary]/6). `reflect_object`
// cannot reach these ([temp.arg.nontype]/6), but `object_of` of a
// reference usable in constant expressions does ([meta.reflection.queries]
// object_of). Such objects are const. GCC 16.1 compiles and runs the same
// file (its type_info object has an internal pseudo type, so the
// type_info checks go through `const void *`).
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: abc 1 y 1 0
//@ stdout: bc 49
#include <meta>
#include <typeinfo>
#include <cstdio>
using namespace std::meta;

constexpr const std::type_info& ti = typeid(int);
constexpr const char (&s)[4] = "abc";
constexpr const char& c1 = "xyz"[1];
constexpr const int& tmp = 42;
constexpr int g = 7;
constexpr const int& rg = g;

static_assert(static_cast<const void*>(&[: object_of(^^ti) :]) == &typeid(int));
static_assert(is_const(object_of(^^ti)));
static_assert([: object_of(^^s) :][1] == 'b');
static_assert(sizeof([: object_of(^^s) :]) == 4);
static_assert(&[: object_of(^^s) :] == &s);
static_assert(type_of(object_of(^^s)) == ^^const char[4]);
static_assert(is_const(object_of(^^s)));
static_assert([: object_of(^^c1) :] == 'y');
static_assert(&[: object_of(^^c1) :] == &c1);
static_assert(type_of(object_of(^^c1)) == ^^const char);
static_assert([: object_of(^^tmp) :] == 42);
static_assert(&[: object_of(^^tmp) :] == &tmp);
static_assert(type_of(object_of(^^tmp)) == ^^const int);
// object_of a reference is the object it refers to.
static_assert(object_of(^^rg) == object_of(^^g));
static_assert(object_of(^^g) == reflect_object(g));
static_assert(type_of(object_of(^^g)) == ^^const int);
static_assert(&[: object_of(^^rg) :] == &g);
// A constant-initialized reference to a non-const variable is usable in
// constant expressions too ([expr.const]/3).
int x;
int& rx = x;
static_assert(object_of(^^rx) == object_of(^^x));
static_assert(!is_const(object_of(^^rx)));
static_assert(&[: object_of(^^rx) :] == &x);

int k() { return [: object_of(^^tmp) :] + [: object_of(^^rg) :]; }

int main() {
  const void* t = &[: object_of(^^ti) :];
  std::printf("%s %d %c %d %d\n", [: object_of(^^s) :], t == &typeid(int),
              [: object_of(^^c1) :], &[: object_of(^^s) :] == &s,
              t == &typeid(long));
  const char* p = [: object_of(^^s) :];
  std::printf("%s %d\n", p + 1, k());
}
