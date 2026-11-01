// Array constants from a braced list without explicit elements have only a
// filler, so clang's `EmitArrayConstant` finds no common element type and
// emits a packed literal struct; a value-initialized array member is
// `EmitNullConstant`'s plain array. Classes with bases flatten the bases'
// fields into the constant (`ConstStructBuilder::Build`).
struct P { int a = 1; };
P gp[3]{};
P gp2[3]{{2}};
int gi[4]{};
struct Pa { char c = 'x'; long l = 2; };
Pa gpa[2]{};
struct Arr { P p[2]; int k[3]; };
Arr garr{};
void use(void*);
void fill() { P p[3]{}; use(p); Arr a{}; use(&a); }
struct T { int x; int T::*p; };
struct C { constexpr C():x(1){} int x; int T::*p = nullptr; };
struct S { int a = 1; int T::*p; };
C gc[3]{};
S gs[3]{};
T gt[3]{{}};
T gu[3] = {};
T gv[3][2]{};
struct M { T t[2]; int y = 2; };
M gm{};
M gm2[2]{};
T gz[10]{{1, &T::x}};

struct A0{int a;}; struct E2: A0{int e;}; E2 g{};
struct A3{int a=1;}; struct E3: A3{int e;}; E3 g3{};
struct A4{long l; char c;}; struct E4: A4{char d;}; E4 g4{{1,2},3};
struct Em{}; struct E5: Em, A3 { int z = 4; }; E5 g5{};
struct E6: A4, A3 {}; E6 g6{{5,6},{7}};
 struct K { constexpr K(): a(3) {} int a; }; struct E7 : K { int b = 1; }; E7 g7{};
