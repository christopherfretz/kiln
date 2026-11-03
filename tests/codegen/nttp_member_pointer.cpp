// Mangled names of specializations whose template argument is a non-null
// pointer to member, including base/derived member-pointer conversions
// (`mc` cast mangling) and data-member reads through the argument;
// conformance in
// tests/conformance/temp.arg.nontype/member-pointer-argument-value.cpp.
struct S { int v; int w; int f() const { return 7; } };
template <int S::*P> struct MP { static int get(S s) { return s.*P; } };
template <int (S::*F)() const> struct MF { static int tag() { return 1; } };
struct B { int b; int g() const { return b + 1; } };
struct D : B { int d; };
template <int D::*P> struct X { static int get(const D &x) { return x.*P; } };
template <int (D::*F)() const> struct Y { static int tag() { return 2; } };
template <int B::*P> struct Z { static int get(const D &x) { return x.*P; } };
constexpr int D::*pb = &B::b;
constexpr int (D::*pg)() const = &B::g;
constexpr int B::*pd = static_cast<int B::*>(&D::d);
template <auto P> int fa() { return 3; }
int run(S s, const D &x) {
  return MP<&S::v>::get(s) + MP<&S::w>::get(s) + MF<&S::f>::tag() + X<pb>::get(x) +
         X<&D::d>::get(x) + Y<pg>::tag() + Z<pd>::get(x) + fa<&S::v>() + fa<pd>();
}
