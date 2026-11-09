// [basic.lookup.elab]/1, [dcl.type.elab]/2-4: valid twin of
// tag-hidden-by-type-name.cpp. A typedef naming a class of the same name in
// its own scope (`typedef struct A A;`) is looked through, non-type names are
// ignored, and a friend class-key naming a type name outside the innermost
// enclosing namespace declares a new class there (WP-957).
//@ std: c++20
struct A { static int foo; };
typedef struct A A;
struct A *p1;
namespace n { struct A {}; typedef struct A A; struct A *p2; }
namespace i2 { int A; int test() { struct A a; return a.foo; } }
namespace i3 { void A(); struct A *p3; }
namespace i4 { template <class T> void A(); struct A *p4; }
struct B {};
namespace m { struct B; typedef B B2; struct B *p5; }
void g() { struct L {}; typedef struct L L; struct L *p6; }
template <class X> struct Y { friend class Y; };
Y<int> y;
template <class> class W;
namespace k { struct S { friend class W; }; }
template <template <class> class TT> struct S7 { friend class TT<int>; };
