// [expr.new]/24, [expr.const]/5.18 (C++26, P2747R2), [intro.object]/9:
// the subobject a `void *` designates survives a store of the `void *` to
// memory (a `void *` member, an array element, a local read back through a
// `void **`, a copied struct), so a placement new through the reloaded
// `void *` constructs that subobject; and an empty base and an empty
// `[[no_unique_address]]` member at one address are distinct designated
// objects, whose construction touches no byte of the member sharing it.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:62 note "placement new would change type of storage from 'int' to 'M'"
//@ diag: 43:15 note "in call to 'member_not_whole()'"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:58 note "placement new would change type of storage from 'G' to 'F'"
//@ diag: 45:15 note "in call to 'nua_member_not_base()'"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:74 note "placement new would change type of storage from 'F' to 'G'"
//@ diag: 47:15 note "in call to 'base_not_nua_member()'"
namespace std { using size_t = decltype(sizeof 0); }
void *operator new(std::size_t, void *p) noexcept;
struct F {};
struct G {};
struct S : F { [[no_unique_address]] G g; int x; };
struct M { int first; int second; };
struct H { void *p; const void *q; };

constexpr int member() { M m{1, 2}; H h{&m.first, &m.second}; new (h.p) int(5); return m.first + m.second; }
static_assert(member() == 7);
constexpr int via_pointer() { M m{1, 2}; void *p = &m.second; void **pp = &p; new (*pp) int(9); return m.second; }
static_assert(via_pointer() == 9);
constexpr int whole_copied() { M m{1, 2}; H h{&m}; H k = h; new (k.p) M{3, 4}; return m.first + m.second; }
static_assert(whole_copied() == 7);
constexpr int element() { M m{1, 2}; void *a[2] = {nullptr, &m.second}; new (a[1]) int(4); return m.second; }
static_assert(element() == 4);
constexpr int nua_member() { S s{}; s.x = 0x1234; H h{&s.g}; new (h.p) G(); return s.x; }
static_assert(nua_member() == 0x1234);
constexpr int empty_base() { S s{}; s.x = 0x1234; H h{static_cast<F *>(&s)}; new (h.p) F(); return s.x; }
static_assert(empty_base() == 0x1234);

// The stored designator is the member, not the enclosing object.
constexpr int member_not_whole() { M m{1, 2}; H h{&m.first}; new (h.p) M{3, 4}; return m.first; }
static_assert(member_not_whole() == 3);
constexpr bool nua_member_not_base() { S s{}; H h{&s.g}; new (h.p) F; return true; }
static_assert(nua_member_not_base());
constexpr bool base_not_nua_member() { S s{}; H h{static_cast<F *>(&s)}; new (h.p) G; return true; }
static_assert(base_not_nua_member());
