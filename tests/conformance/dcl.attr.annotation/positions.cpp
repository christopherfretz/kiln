//@ expect: ast
//@ skip(clang): clang 19 does not implement P3394 annotations
// [dcl.attr.annotation]/1 (C++26 P3394R4): an annotation may appertain to
// a declaration of a type, type alias, variable, function, function
// parameter, namespace, enumerator or non-static data member (bit-fields
// included), at every position of an attribute-specifier-seq that
// appertains to that declaration. Each is evaluated to its underlying
// constant ([dcl.attr.annotation]/2) and kept in the order written.
struct Tag { int id; };
[[=1]] int leading;
int trailing [[=2]];
[[=3]] int both [[=4]];
[[=Tag{5}]] void function();
void parameters([[=6]] int a, int b [[=7]]);
namespace [[=8]] ns { }
using Alias [[=9]] = int;
typedef int Typedef [[=10]];
struct [[=11]] Record {
  [[=12]] int member;
  int bits [[=13]] : 3;
  [[=14]] static int static_member;
  [[=15]] void method();
  [[=16]] Record();
  [[=17]] friend void defined_friend() { }
};
enum [[=18]] Enum { enumerator [[=19]] };
enum class [[=20]] Scoped : int { x };
void locals() {
  [[=21]] int local = 0;
  [[=22]] static int static_local;
  (void)local;
}
struct Pair { int x, y; };
[[=23]] auto [px, py] = Pair{1, 2};
[[=24.5]] double floating;
[[='c']] [[=nullptr]] int other_types;
constexpr int target = 0;
[[=&target]] int pointer;
