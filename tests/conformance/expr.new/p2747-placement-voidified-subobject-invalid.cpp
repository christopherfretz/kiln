// [expr.new]/24, [expr.const]/5.18 (C++26, P2747R2): a `void *` made from a
// pointer to a subobject designates that subobject, not the enclosing
// object at the same address, so a placement new of the enclosing
// object's type there is not a constant expression; and a `void *` made
// from the whole object still designates the whole object, not its
// first member (clang 22: "placement new would change type of storage").
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:46 note "placement new would change type of storage from 'int' to 'S'"
//@ diag: 29:15 note "in call to 'whole_at_member()'"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:46 note "placement new would change type of storage from 'S' to 'int'"
//@ diag: 31:15 note "in call to 'member_at_whole()'"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:42 note "placement new would change type of storage from 'B' to 'D'"
//@ diag: 33:15 note "in call to 'derived_at_base()'"
namespace std { using size_t = decltype(sizeof 0); }
void *operator new(std::size_t, void *p) noexcept;

struct S { int first; int second; };
struct B { int v; };
struct D : B { int w; };
constexpr void *first_of(S &s) { return &s.first; }
constexpr void *whole(S &s) { return &s; }
constexpr void *base_of(D &d) { return static_cast<B *>(&d); }
constexpr int whole_at_member() { S s{1, 2}; new (first_of(s)) S{3, 4}; return s.first; }
static_assert(whole_at_member() == 3);
constexpr int member_at_whole() { S s{1, 2}; new (whole(s)) int(5); return s.first; }
static_assert(member_at_whole() == 5);
constexpr int derived_at_base() { D d{}; new (base_of(d)) D{}; return d.w; }
static_assert(derived_at_base() == 0);
