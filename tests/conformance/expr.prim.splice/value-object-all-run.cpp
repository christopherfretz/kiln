// [expr.prim.splice]/2 (P2996R13): every value reflection splices to a
// prvalue of its value and every object reflection to an lvalue
// designating its object or subobject (WP-1743). Values: each family of
// structural scalar (integers of every width, bool, characters,
// enumerations, floating-point, null pointers, pointers to a function, a
// member, an element, member pointers, a reflection), a class, a union, a
// closure and a template argument. Objects: a member, a base and a member
// of one, an element of a nested array, an empty [[no_unique_address]]
// member, anonymous union and anonymous struct members, a static member,
// a static local of a consteval function, a subobject of a template
// parameter object, and members reached through a private base and a
// private member (a splice checks no access). Stores through the object
// splices are seen by the program. GCC 16.1 compiles and runs the same
// file with the same output.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: 42 1 q z 4 200 2.5 1.25 3.5
//@ stdout: 1 1 8 1 3 9
//@ stdout: 3 5 1
//@ stdout: 8 2 10
//@ stdout: 1 255 17
//@ stdout: 1 12 13 4 5 6 1
//@ stdout: 3 3 6 11
//@ stdout: 7 8 5
//@ stdout: 1 1
#include <meta>
#include <cstdio>
using namespace std::meta;

struct S { int a; int b[3]; };
union U { int i; float f; };
enum E { e1 = 4 };
enum class EC : unsigned char { x = 200 };
int twice(int x) { return x * 2; }
constexpr S gs{1, {2, 3, 4}};
constexpr int garr[3] = {7, 8, 9};
struct M { int m; int g(int) const { return 5; } };
constexpr auto lam = [](int x) { return x + 1; };
template <auto V> constexpr auto tv = V;

// Value reflections of every structural scalar family, a class, a union
// and a closure.
constexpr auto v_int = reflect_constant(42);
constexpr auto v_bool = reflect_constant(true);
constexpr auto v_char = reflect_constant('q');
constexpr auto v_u8 = reflect_constant(u8'z');
constexpr auto v_e = reflect_constant(e1);
constexpr auto v_ec = reflect_constant(EC::x);
constexpr auto v_d = reflect_constant(2.5);
constexpr auto v_f = reflect_constant(1.25f);
constexpr auto v_ld = reflect_constant(3.5L);
constexpr auto v_null = reflect_constant(nullptr);
constexpr auto v_nullp = reflect_constant((int*)nullptr);
constexpr auto v_fp = reflect_constant(&twice);
constexpr auto v_pa = reflect_constant(&gs.a);
constexpr auto v_pb = reflect_constant(&gs.b[1]);
constexpr auto v_parr = reflect_constant(garr);
constexpr auto v_mp = reflect_constant(&M::m);
constexpr auto v_mfp = reflect_constant(&M::g);
constexpr auto v_nmp = reflect_constant((int M::*)nullptr);
constexpr auto v_refl = reflect_constant(^^int);
constexpr auto v_cls = reflect_constant(S{5, {6, 7, 8}});
constexpr auto v_u = reflect_constant(U{.f = 2.0f});
constexpr auto v_lam = reflect_constant(lam);
constexpr auto v_i128 = reflect_constant((__int128)1 << 100);
constexpr auto v_u128 = reflect_constant(~(unsigned __int128)0);
constexpr auto v_targ = template_arguments_of(^^tv<17>)[0];
static_assert([:v_refl:] == ^^int);

// Object reflections: a member, a base, a member of a base, an element
// of a nested array, an empty [[no_unique_address]] member, an anonymous
// union member, a member of an anonymous struct, a static member, a
// static local of a consteval function, a subobject of a template
// parameter object, and members reached through a private base and a
// private member (a splice checks no access).
struct B { int bv; };
struct VB { int vv; };
struct E0 {};
struct D : B, VB {
  int d;
  int arr[2][2];
  [[no_unique_address]] E0 e;
  union { int ui; float uf; };
  struct { int x, y; };
};
constinit D gd{};
struct H { static constexpr int sm = 3; };
consteval info local_static() { static constexpr int s = 11; return reflect_object(s); }
template <S P> struct TP { static constexpr auto r = reflect_object(P.b[1]); };
class P { int hidden = 4; public: int shown = 5; };
class Q : P { public: int q = 1; };
constinit P gp{};
constinit Q gq{};
consteval info priv() { return nonstatic_data_members_of(^^P, access_context::unchecked())[0]; }
consteval info pbase() { return bases_of(^^Q, access_context::unchecked())[0]; }

int main() {
  std::printf("%d %d %c %c %d %d %g %g %Lg\n", [:v_int:], [:v_bool:], [:v_char:], (char)[:v_u8:],
              (int)[:v_e:], (int)[:v_ec:], [:v_d:], (double)[:v_f:], [:v_ld:]);
  std::printf("%d %d %d %d %d %d\n", [:v_null:] == nullptr, [:v_nullp:] == nullptr, [:v_fp:](4),
              *[:v_pa:], *[:v_pb:], [:v_parr:][2]);
  std::printf("%d %d %d\n", M{3}.*[:v_mp:], (M{}.*[:v_mfp:])(1), [:v_nmp:] == nullptr);
  std::printf("%d %g %d\n", [:v_cls:].b[2], (double)[:v_u:].f, [:v_lam:](9));
  std::printf("%d %d %d\n", (int)([:v_i128:] >> 100), (int)([:v_u128:] & 0xff), [:v_targ:]);

  constexpr auto o_d = reflect_object(gd.d);
  constexpr auto o_b = reflect_object(static_cast<B&>(gd));
  constexpr auto o_bv = reflect_object(gd.bv);
  constexpr auto o_vb = reflect_object(static_cast<VB&>(gd));
  constexpr auto o_vv = reflect_object(gd.vv);
  constexpr auto o_arr = reflect_object(gd.arr[1][0]);
  constexpr auto o_e = reflect_object(gd.e);
  constexpr auto o_ui = reflect_object(gd.ui);
  constexpr auto o_y = reflect_object(gd.y);
  constexpr auto o_gsb = reflect_object(gs.b);
  constexpr auto o_sm = reflect_object(H::sm);
  constexpr auto o_tp = TP<S{4, {5, 6, 7}}>::r;
  constexpr auto o_h = reflect_object(gp.[:priv():]);
  constexpr auto o_qb = reflect_object(gq.[:pbase():]);
  constexpr auto o_qbh = reflect_object(gq.[:pbase():].[:priv():]);
  [:o_d:] = 1;
  [:o_bv:] = 2;
  [:o_vv:] = 3;
  [:o_arr:] = 4;
  [:o_ui:] = 5;
  [:o_y:] = 6;
  [:o_b:].bv += 10;
  [:o_vb:].vv += 10;
  [:o_h:] = 7;
  [:o_qbh:] = 8;
  std::printf("%d %d %d %d %d %d %d\n", gd.d, gd.bv, gd.vv, gd.arr[1][0], gd.ui, gd.y,
              (int)sizeof([:o_e:]));
  std::printf("%d %d %d %d\n", [:o_gsb:][1], [:o_sm:], [:o_tp:], [:local_static():]);
  std::printf("%d %d %d\n", gp.[:priv():], gq.[:pbase():].[:priv():], [:o_qb:].shown);
  std::printf("%d %d\n", &[:o_d:] == &gd.d, &[:o_arr:] == &gd.arr[1][0]);
}
