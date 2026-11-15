// The padding of a C constant aggregate (clang 20+
// `CodeGenModule::shouldZeroInitPadding`, C only, every standard): each
// gap before a member and the tail are explicit zero bytes, so a struct
// with padding is a literal struct `{ i8, [3 x i8], i32 }`, not the named
// type; a union's bytes past its initialized member are zero. A member,
// element or object initialized by nothing (an array filler, an omitted
// member, a tentative definition) is the null of the named type (clang
// `EmitNullConstant`). clang 19 (KILN_ORACLE=19) leaves the padding
// `undef` and keeps the named type. A bit-field storage unit is one `i8`
// per byte its named fields cover (`ConstantAggregateBuilder::addBits`).
// C23 6.7.11p11; spec/codegen.md "Static initialization".
//@ expect: ir
//@ args: -std=gnu17
struct S { char c; int i; };
struct T { int i; char c; };
struct N { char a; struct S s; short z; };
union U { char c; int i; };
union V { int i; long long l; };
struct B { char a; int b : 3; int : 0; char d; };
struct BF { int a : 3, b : 20; char c; };
struct BF4 { int a : 8, b : 8; char c; };
struct O { int z; struct S s; struct S t[2]; };
struct Pk { char c; int i; } __attribute__((packed));
struct Fl { int n; char d[]; };
struct S g1 = {1, 2};
struct T g2 = {1, 2};
struct N g3 = {1, {2, 3}, 4};
union U g4 = {1};
union V g5 = {1};
struct S g6 = {0};
const struct S g7 = {};
union V g8 = {};
struct S g9[2] = {{1, 2}, {3, 4}};
struct S g10[3] = {{1, 2}};
struct S g11[4] = {[2] = {1, 2}};
struct B g12 = {1, 2, 3};
struct BF g13 = {1, 2, 3};
struct BF4 g14 = {1, 2, 3};
struct O g15 = {.z = 1};
struct O g16 = {1, {}, {{1, 2}}};
struct Pk g17 = {1, 2};
struct Fl g18 = {1, {2, 3}};
struct S *g19 = &(struct S){5, 6};
extern struct S g20;
struct S g20 = {7};
int f(void) {
  static struct T st = {3, 4};
  struct S l = {1, 2};
  return st.i + l.i;
}
