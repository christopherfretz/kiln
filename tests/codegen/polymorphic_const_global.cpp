// [basic.start.static], [class.virtual]: a constant-initialized global of a
// polymorphic class is a constant initializer whose vptr fields hold the
// vtable address point of each subobject (the complete object's vtable
// group for a base subobject, clang `ConstantEmitter`); an array of such
// objects is an array of their common element type. The vtables stay
// external (undefined key functions), so no RTTI is emitted.
struct P { virtual int f() const; int x; constexpr P(int x) : x(x) {} };
struct Q { virtual long g() const; long q = 4; };
struct R : P, Q { int r = 5; constexpr R() : P(7) {} int f() const override; };
struct S { int a = 1; R r; P ps[2] = {P(1), P(2)}; };
struct E : R { char e = 'e'; constexpr E() = default; long g() const override; };
constexpr P p(3);
R r;
const R cr;
S s;
E e;
P arr[3] = {P(4), P(5), P(6)};
const P *use() { return &p; }
const R *use2() { return &cr; }
