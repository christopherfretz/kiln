// [intro.object]/9, [dcl.attr.nouniqueaddr]/2, [class.bit]/1 (WP-294):
// empty [[no_unique_address]] members of distinct types overlap each
// other and the following members (same-type ones get distinct
// addresses); bit-field storage units after them, in a potentially
// overlapping member's tail padding, and around them. Exercises the
// layout, and (through codegen) the record IR type's insertPadding.
struct E {};
struct F {};
struct G { E e; };
struct NP { int i; char c; NP() = default; NP(const NP&) {} };  // non-POD
struct A { [[no_unique_address]] E e; [[no_unique_address]] F f; int b : 3; int c : 5; };
struct B { int a : 4; [[no_unique_address]] E e; [[no_unique_address]] F f; int b : 4; };
struct C { [[no_unique_address]] E e1; [[no_unique_address]] E e2; char x : 2; char y : 7; };
struct D { [[no_unique_address]] NP np; char c : 3; char d : 6; [[no_unique_address]] E e; };
struct H { [[no_unique_address]] E e; [[no_unique_address]] G g; long l : 40; short s : 9; };
struct I { char c; [[no_unique_address]] E e; [[no_unique_address]] F f; long long x : 33; };
struct J { [[no_unique_address]] NP np; [[no_unique_address]] E e; [[no_unique_address]] F f; char t; };
struct K : E { [[no_unique_address]] E e; [[no_unique_address]] F f; unsigned u : 1; };
A a; B b; C c; D d; H h; I i; J j; K k;
