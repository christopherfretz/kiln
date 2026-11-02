// The record IR types of overlapping empty [[no_unique_address]] members
// and bit-field access units (clang CGRecordLowering insertPadding:
// members with storage never overlap; spec/abi.md §6). [dcl.attr.nouniqueaddr]/2,
// [class.bit]/1 (WP-294; the layouts are in tests/layout/nua_overlap_bitfields.cpp).
struct E {};
struct F {};
struct G { E e; };
struct NP { int i; char c; NP() = default; NP(const NP&) {} };
struct A { [[no_unique_address]] E e; [[no_unique_address]] F f; int b : 3; int c : 5; };
struct B { int a : 4; [[no_unique_address]] E e; [[no_unique_address]] F f; int b : 4; };
struct C { [[no_unique_address]] E e1; [[no_unique_address]] E e2; char x : 2; char y : 7; };
struct D { [[no_unique_address]] NP np; char c : 3; char d : 6; [[no_unique_address]] E e; };
struct H { [[no_unique_address]] E e; [[no_unique_address]] G g; long l : 40; short s : 9; };
struct I { char c; [[no_unique_address]] E e; [[no_unique_address]] F f; long long x : 33; };
struct J { [[no_unique_address]] NP np; [[no_unique_address]] E e; [[no_unique_address]] F f; char t; };
struct K : E { [[no_unique_address]] E e; [[no_unique_address]] F f; unsigned u : 1; };
A a; B b; C c; D d; H h; I i; J j; K k;
int touch() {
  a.c = 2; b.b = 3; c.y = 4; d.d = 5; h.s = 6; i.x = 7; j.t = 8; k.u = 1;
  return a.b + b.a + c.x + d.c + (int)h.l + (int)i.x + j.np.i + (int)k.u;
}
