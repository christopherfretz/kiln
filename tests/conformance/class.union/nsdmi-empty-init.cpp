// [dcl.init.aggr]/5: a union initialized by an empty braced list
// initializes the variant member that has a default member initializer
// (directly, or within an anonymous struct member); without one, the first
// member is value-initialized. Constant evaluation reads that member.
//@ expect: run
//@ strict-diags
union N { int i; float f = 3.0f; };
union Z { int i; float f; };
struct M { int x = 4; union { int i; float f = 2.0f; }; };
struct W { union { struct { int a; int b = 6; }; long l; }; };
static_assert(N{}.f == 3.0f);
static_assert(M{}.f == 2.0f && M{}.x == 4);
static_assert(W{}.b == 6 && W{}.a == 0);
static_assert(Z{}.i == 0);
constexpr N n{};
static_assert(n.f == 3.0f);
int main() {
  N a{};
  M m = {};
  W w{};
  return (a.f == 3.0f && m.f == 2.0f && w.b == 6) ? 0 : 1;
}
