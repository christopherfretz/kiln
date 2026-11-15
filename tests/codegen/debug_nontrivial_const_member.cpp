// Debug info (WP-1445): a class with a const member of class type is
// DIFlagNonTrivial, as in clang, whose isTrivial() counts the implicit
// copy assignment that selects no member for the const subobject as
// non-trivial. The type traits follow [class.prop]/1 instead (such a
// class is trivially copyable, docs/status/deviations.md); only the flag
// follows clang. A const scalar member does not do this.
//@ args: -g
struct E {};
struct I { int i; };
struct C1 { const E e; };
struct C2 { const I i; int j; };
struct C3 { const E e[2]; };
struct C4 { const int x; };
struct C5 { const I i; C5() = default; };
int use(C1 a, C2 &b, C3 c, C4 &d, C5 &e) { return b.j + d.x + e.i.i; }
