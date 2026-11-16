// [class.union.general]/6, [expr.const]/5.10: after a placement new of
// one of two union members of one type at one offset, reading the other
// is not constant: `&s.b` designates `b` (WP-1494, spec/oven.md "Union
// member designators").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:78 note "read of member 'a' of union with active member 'b' is not allowed in a constant expression"
//@ diag: 20:15 note "in call to 'read_other()'"
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:92 note "read of member 'p' of union with active member 'q' is not allowed in a constant expression"
//@ diag: 24:15 note "in call to 'read_other_union()'"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
} // namespace std
union S { int a; int b; };
constexpr int read_other() { S s{.a = 1}; std::construct_at(&s.b, 7); return s.a; }
static_assert(read_other() == 7);
union V { int x; float y; };
union W { V p; V q; char c; };
constexpr int read_other_union() { W w{.c = 1}; std::construct_at(&w.q, V{.x = 3}); return w.p.x; }
static_assert(read_other_union() == 3);
