// [WP-438] Arrays of `bool` ext vectors with constant initialisers
// (clang ext_vector_type; [dcl.init.aggr], [expr.const]; clang
// ConstExprEmitter `EmitArrayInitialization` / `EmitArrayConstant`): the
// explicit elements are `<N x i1>` constants, a value-initialized filler
// element and an all-zero array are of the memory form (`i8` for
// `<4 x i1>`, `[2 x i8]` for a row), so a partly initialized array is a
// packed struct; nested arrays, class members and value-initialized
// class elements likewise. No `__cxx_global_var_init`.
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef bool b32 __attribute__((ext_vector_type(32)));
typedef int i4 __attribute__((ext_vector_type(4)));
b4 a1[2] = {{true}, {false, true}};
b4 a2[4] = {{true}, {false, true}};
b4 a3[2][2] = {{{true}, {true}}, {{true}, {true}}};
b4 a4[2][2] = {{{true}, {true}}};
b4 a5[12] = {{true}};
b4 a6[3] = {{true}, {false}, {true}};
b16 a7[3] = {{true}};
b4 a8[2] = {};
b4 a9[3] = {{}, {true}};
struct X { b4 a[2]; int i; } x1 = {{{true}}, 0};
struct Y { b4 a[2]; int i; } y1 = {{{true}, {true}}, 0};
struct Z { b4 a[2][2]; } z1 = {{{{true}}}};
constexpr b4 K[3] = {{true}, {false, true}};
b4 a10[3] = {K[0], K[1]};
b4 e1[3] = {{false}};
b4 e2[3] = {{false}, {false}, {false}};
b4 e3[20] = {{true}, {true}};
b4 e4[10][2] = {{{true}}};
b32 e5[2] = {{true}};
struct P { b4 v; int i; };
P e6[3] = {{{true}, 1}};
struct Q { b4 a[3] = {}; int i; } e7 = {.i = 2};
struct R { b4 a[3]; } e8 = {};
struct S { int i; b4 a[2]; } e9 = {1};
i4 e10[3] = {{1}};
const b4 *use() { static b4 l[3] = {{true}}; return l; }
constexpr b4 e11[2][3] = {{{true}}, {{false}, {true}}};
const void *pe = &e11;
struct X2 { b4 a[3]; int i; };
constexpr X2 mk() { X2 x{}; x.a[0] = (b4)true; return x; }
X2 x2 = mk();
constexpr X2 mk2() { return {{(b4)true}, 1}; }
X2 x3 = mk2();
struct W { b4 v; int i; };
constexpr W mkw() { return {}; }
W w1 = mkw();
W w2[2] = {mkw(), {(b4)true, 2}};
