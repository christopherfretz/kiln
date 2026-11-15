// [class.bit]: a constant whose class has bit-fields stores each
// bit-field storage unit as one `i8` per byte its named, non-empty
// bit-fields cover (clang `ConstStructBuilder::AppendBitField` through
// `ConstantAggregateBuilder::addBits`), so a storage unit wider than a
// byte makes the constant a literal struct of bytes; a byte no field
// covers is padding. spec/codegen.md "Static initialization".
//@ expect: ir
struct A { int a : 3, b : 20; char c; };
struct B { int a : 8, b : 8; char c; };
struct C { char a; long long b : 40; short s; };
struct D { int a : 4, : 12, b : 4; };
struct E { char c; int a : 3; };
A a = {1, 2, 3};
B b = {1, 2, 3};
C c = {1, 2, 3};
D d = {1, 2};
E e = {1, 2};
constexpr B cb = {4, 5, 6};
const B *pb = &cb;
int get() { return a.c + pb->c + e.a; }
