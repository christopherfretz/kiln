// [meta.reflection.extract]/12: extract by value is
// `extract-value<U>(constant_of(r))`, and constant_of(r) is
// `reflect_constant([: R :])` ([meta.reflection.queries]): for a
// structured binding ([dcl.struct.bind]/1, a name of a member or element
// of the hidden decomposition variable e) or an automatic variable whose
// lifetime began within the constant evaluation, the splice reads it
// there ([expr.const]/10: an object whose lifetime began within the
// evaluation; constant_of's example rejects only an object outside its
// lifetime). [: ^^a :] of a binding in its scope is a valid
// splice-expression ([expr.prim.splice]/2). So all of these are
// constants, read as they are when extracted. Reduced from real code;
// GCC 16.1 agrees.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>

using std::meta::constant_of;
using std::meta::extract;
using std::meta::reflect_constant;

struct P { int a; int b; };

// By value: the bindings name the members of the copy e.
consteval int by_value() {
  auto [a, b] = P{1, 2};
  return extract<int>(^^a) * 10 + extract<int>(^^b);
}
static_assert(by_value() == 12);

consteval int splice() {
  auto [a, b] = P{3, 4};
  return [:^^a:] * 10 + [:^^b:];
}
static_assert(splice() == 34);

// Read as it is when extracted, not as it was initialized.
consteval int after_write() {
  auto [a, b] = P{1, 2};
  a = 5;
  return extract<int>(^^a) * 10 + extract<const int>(^^b);
}
static_assert(after_write() == 52);

// A reference decomposition: the bindings name the members of p.
consteval int by_reference() {
  P p{5, 6};
  auto& [a, b] = p;
  p.a = 7;
  return extract<int>(^^a) * 10 + [:^^b:];
}
static_assert(by_reference() == 76);

// An array's elements; a base class's member; a member of class type.
consteval int array() {
  int arr[2] = {7, 8};
  auto& [x, y] = arr;
  arr[1] = 9;
  return extract<int>(^^x) * 10 + extract<int>(^^y);
}
static_assert(array() == 79);

struct B { int x; };
struct D : B {};
consteval int base() {
  auto [x] = D{{3}};
  return extract<int>(^^x);
}
static_assert(base() == 3);

struct Q { P p; int c; };
consteval int class_member() {
  auto [p, c] = Q{{1, 2}, 3};
  return extract<P>(^^p).b * 10 + c;
}
static_assert(class_member() == 23);

// constant_of of a binding, of a local and of a parameter: their value now.
consteval bool constants(int n) {
  auto [a, b] = P{1, 2};
  int x = 3;
  b = 4;
  return constant_of(^^a) == reflect_constant(1) && constant_of(^^b) == reflect_constant(4) &&
         constant_of(^^x) == reflect_constant(3) && extract<int>(constant_of(^^n)) == n;
}
static_assert(constants(9));

// A class-type local: its value, frozen.
consteval int class_local() {
  P p{5, 6};
  return extract<P>(constant_of(^^p)).b;
}
static_assert(class_local() == 6);

// A binding of a decomposition in a template; the one before a pack.
template <class T> consteval int templ() {
  auto [x, ...xs] = T{1, 2};
  return [:^^x:] * 10 + extract<int>(^^x);
}
static_assert(templ<P>() == 11);
