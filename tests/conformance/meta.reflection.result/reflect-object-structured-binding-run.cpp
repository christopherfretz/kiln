// [meta.reflection.result] reflect_object of a structured binding
// (P2996R13, [dcl.struct.bind]/5-7): the object reflection designates the
// element or member of the decomposed variable e (or the object a
// tuple-like binding refers to), so its splice is an lvalue naming that
// object: a store through it changes what the binding names, and code
// generation emits its address as e's (WP-1783). GCC 16.1 rejects
// reflect_object of a structured binding (docs/status/deviations.md).
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ stdout: 7 2 1
//@ stdout: 0 9 1
//@ stdout: 4 1
//@ stdout: 30 1
#include <meta>
#include <cstdio>
#include <tuple>
using namespace std::meta;

struct P { int x; int y; };
P gp{1, 2};
auto [a, b] = gp;
int arr[2] = {5, 6};
auto [e0, e1] = arr;
constexpr P cp{3, 4};
constexpr auto [ca, cb] = cp;
std::tuple<int, long> gt{20, 30};
auto& [t0, t1] = gt;

constexpr info ra = reflect_object(a);
constexpr info re1 = reflect_object(e1);
constexpr info rcb = reflect_object(cb);
constexpr info rt1 = reflect_object(t1);
int* volatile pa = &[:ra:];
const int* volatile pcb = &[:rcb:];

int main() {
  [:ra:] = 7;
  std::printf("%d %d %d\n", a, b, pa == &a);
  [:re1:] = 9;
  std::printf("%d %d %d\n", arr[1] - 6, e1, &[:re1:] == &e1);
  std::printf("%d %d\n", *pcb, pcb == &cb);
  long v = [:rt1:];
  std::printf("%ld %d\n", v, &[:rt1:] == &std::get<1>(gt));
}
