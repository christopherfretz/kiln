// [class.base.init]/2: a qualified mem-initializer-id is looked up as a
// type; a non-static data member is named unqualified only. A qualified
// name that names no type is clang's "member initializer ... does not
// name a non-static data member or base class" at the name (WP-1745).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:32 error "member initializer 'y' does not name a non-static data member or base class"
//@ diag: 14:28 error "member initializer 'Q' does not name a non-static data member or base class"
//@ diag: 15:25 error "member initializer 'x' does not name a non-static data member or base class"

struct B { B(int); int x; };
namespace N { struct M { M(int); }; }
struct D : B { int y; D() : D::y(1), B(2) {} };
struct E : N::M { E() : N::Q(1) {} };
struct I : B { I() : I::x(1), B(1) {} };
struct F : B { F() : F::B(1) {} };
