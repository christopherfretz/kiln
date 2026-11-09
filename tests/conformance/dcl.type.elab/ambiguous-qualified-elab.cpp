// [basic.lookup.elab]/1, [namespace.udir]/6: a qualified elaborated-type-
// specifier that finds the class through two using-directives is ambiguous.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, ambiguous qualified elaborated lookup is not diagnosed
//@ expect: compile-fail
//@ diag: 14:12 error "reference to 'x' is ambiguous"
namespace A { struct x; }
namespace B { struct x; }
namespace AB {
using namespace A;
using namespace B;
}
struct AB::x *foo;
