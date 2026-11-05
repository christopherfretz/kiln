// [meta.reflection.extract] (P2996R13), spec/reflection-values.md §5:
// extract<T&>(r) (extract-ref) is a glvalue immediate invocation whose
// result is the object r designates itself ([expr.const]/22): a
// reference bound to it, constexpr or not, global or local, holds that
// object's address, compared here both in static_asserts and at run time.
// The object need not be usable in constant expressions (a non-const
// global is only designated, never read); a class-type, array or
// subobject result and reflect_object / object_of / constant_of operands
// behave alike, and an extracted reference may be read inside a larger
// constant expression (WP-640).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: run
//@ strict-diags
//@ exit: 63
#include "kiln_meta.h"
#include "kiln_meta_queries.h"
using namespace std::meta;

constexpr int g = 21;
int h = 5;
struct P { int x, y; };
constexpr P cp{3, 4};
P mp{7, 8};
int arr[] = {1, 2, 3};
struct S { static constexpr int k = 9; };
template <int N> struct A { static constexpr int v = N; };

constexpr const int& r1 = extract<const int&>(^^g);
static_assert(&r1 == &g && r1 == 21);
static_assert(&extract<const int&>(^^g) == &g);
constexpr int& r2 = extract<int&>(^^h);
static_assert(&r2 == &h);
constexpr const int& r3 = extract<const int&>(reflect_object(g));
static_assert(&r3 == &g);
constexpr int& r4 = extract<int&>(reflect_object(arr[1]));
static_assert(&r4 == &arr[1]);
constexpr const P& r5 = extract<const P&>(^^cp);
static_assert(&r5 == &cp && r5.y == 4);
constexpr P& r6 = extract<P&>(^^mp);
static_assert(&r6 == &mp);
constexpr int& r7 = extract<int&>(reflect_object(mp.y));
static_assert(&r7 == &mp.y);
constexpr const int& r8 = extract<const int&>(object_of(^^g));
static_assert(&r8 == &g);
constexpr const int& r9 = extract<const int&>(object_of(^^S::k));
static_assert(&r9 == &S::k && r9 == 9);
constexpr const int (&r10)[3] = extract<const int (&)[3]>(^^arr);
static_assert(&r10[2] == &arr[2]);
static_assert(extract<int>(constant_of(^^g)) == 21);
static_assert(extract<const int&>(^^g) + 1 == 22);
constexpr int v1 = extract<const int&>(^^g) * 2;
static_assert(v1 == 42);
static_assert(A<extract<const int&>(^^g)>::v == 21);
static_assert(&extract<const int&>(^^S::k) == &S::k);

const int& n1 = extract<const int&>(^^g);
P& n2 = extract<P&>(^^mp);

int main() {
  static constexpr P sp{5, 6};
  constexpr const int& l1 = extract<const int&>(^^g);
  const int& l2 = extract<const int&>(^^g);
  int& l3 = extract<int&>(^^h);
  P& l4 = extract<P&>(^^mp);
  const P& l5 = extract<const P&>(reflect_object(sp));
  P copy = extract<const P&>(^^cp);
  l3 = 6;
  l4.x = 9;
  int bits = 0;
  bits |= (&r1 == &g && &r2 == &h && &r3 == &g && &r4 == &arr[1]) << 0;
  bits |= (&r5 == &cp && &r6 == &mp && &r7 == &mp.y && &r8 == &g) << 1;
  bits |= (&n1 == &g && &n2 == &mp && &l1 == &g && &l2 == &g) << 2;
  bits |= (h == 6 && r2 == 6 && mp.x == 9 && r6.x == 9) << 3;
  bits |= (&l5 == &sp && l5.y == 6 && copy.x == 3 && &r10[0] == &arr[0]) << 4;
  bits |= (r1 + r9 + r5.y == 34) << 5;
  return bits;
}
