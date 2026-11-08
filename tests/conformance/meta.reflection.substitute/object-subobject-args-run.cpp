// [meta.reflection.substitute] (P2996R13), [temp.arg.nontype]/2: an object
// reflection used as a template argument of `substitute` stands for the
// object it designates at any offset: a data member, an array element
// inside a member, a base class subobject of a constexpr variable, an
// element of an array variable, and a reflect_constant_array object
// (P3491R3) bound to a reference to array or decayed to a pointer. The
// specialization equals the one the subobject names directly, and is
// emitted and called. GCC 16.1 compiles and runs the same file with the
// same output.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: 30 10 2 3 5
#include <cstdio>
#include <meta>
#include <vector>

struct B { int z = 7; };
struct S : B { int a; int b; int m[2]; };
template <const int& R> constexpr int get() { return R; }
template <const int* P> constexpr const int* addr() { return P; }
template <const B& R> constexpr int getb() { return R.z; }
template <const int (&A)[3]> constexpr int third() { return A[2]; }

constexpr S s{{9}, 1, 2, {3, 4}};
constexpr int arr[3] = {4, 5, 6};
constexpr auto ra = std::meta::reflect_constant_array(std::vector<int>{10, 20, 30});

using std::meta::reflect_object;
using std::meta::substitute;
static_assert([: substitute(^^get, {reflect_object(s.b)}) :]() == 2);
static_assert([: substitute(^^get, {reflect_object(s.m[1])}) :]() == 4);
static_assert([: substitute(^^getb, {reflect_object(static_cast<const B&>(s))}) :]() == 9);
static_assert(substitute(^^get, {reflect_object(s.b)}) == ^^get<s.b>);
static_assert([: substitute(^^get, {reflect_object(arr[2])}) :]() == 6);
static_assert(*[: substitute(^^addr, {ra}) :]() == 10);
static_assert([: substitute(^^third, {ra}) :]() == 30);

int main() {
  std::printf("%d %d %d ", [: substitute(^^third, {ra}) :](), *[: substitute(^^addr, {ra}) :](),
              [: substitute(^^get, {reflect_object(s.b)}) :]());
  std::printf("%d %d\n", [: substitute(^^get, {reflect_object(s.m[0])}) :](),
              [: substitute(^^get, {reflect_object(arr[1])}) :]());
}
