// [basic.lookup.elab]/1, [namespace.udir]/6: a qualified elaborated-type-
// specifier that finds two classes (or enums) through two using-directives is
// ambiguous; one using-directive, the same one twice, or a using-declaration
// of the same entity stay valid. Found by the clang negative-test run
// (WP-912), fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:12 error "reference to 'x' is ambiguous"
//@ diag: 16:22 note "candidate found by name lookup is 'B::x'"
//@ diag: 15:22 note "candidate found by name lookup is 'A::x'"
//@ diag: 22:10 error "reference to 'e' is ambiguous"
//@ diag: 16:30 note "candidate found by name lookup is 'B::e'"
//@ diag: 15:30 note "candidate found by name lookup is 'A::e'"
namespace A { struct x; enum e : int; }
namespace B { struct x; enum e : int; }
namespace AB {
using namespace A;
using namespace B;
}
struct AB::x *foo;
enum AB::e *bar;
namespace C { using namespace A; }
struct C::x *ok1;
namespace D { using namespace A; using namespace A; }
struct D::x *ok2;
namespace E { using A::x; using namespace A; }
struct E::x *ok3;
