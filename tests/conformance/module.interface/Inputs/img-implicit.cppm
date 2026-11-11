// Classes whose implicit special members need overload resolution, for
// the declaration image (spec/modules.md §11.11 "Implicit members",
// WP-1292).
export module imgim;
export namespace im {
struct B { int b = 1; ~B() {} };
struct D : B { int d = 2; };
struct M { M() {} M(M &&) {} M &operator=(M &&) { return *this; } };
struct X { M m; int x = 3; };
struct C { C() {} C(const C &) { } int c = 4; };
struct Y { C c; Y() = default; };
struct DelD { ~DelD() = delete; };
struct Z { int z; Z() {} Z(const Z &) = default; Z(Z &&) {} };
struct W { Z z; };
struct T { const int t = 5; };
struct U { T t; };
struct P { private: ~P() {} friend struct Q; };
struct Q { P p; };
struct V { V(V &) {} V() {} };
struct VV { V v; };
} // namespace im
