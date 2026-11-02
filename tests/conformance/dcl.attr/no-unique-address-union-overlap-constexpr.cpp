// [intro.object]/9, [dcl.attr.nouniqueaddr]/2, [class.union.general]/6,
// [expr.const]/5.10: a union of empty `[[no_unique_address]]` members holds
// no data, so a `[[no_unique_address]]` member after it shares its offset
// (libc++'s `expected<void, E>`: its `__has_val_` flag over the union of
// `__empty_` and `__unex_`). Initializing, writing, constructing or reading
// that member is not an access through the union: the union-member walk
// follows the object accessed, not the byte (WP-334). An access to an
// inactive member of the union itself is still not constant.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:51 note "member call on member 'f' of union with active member 'e' is not allowed in a constant expression"
//@ diag: 49:15 note "in call to 'inactive_call()'"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:47 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 51:15 note "in call to 'flag_uninit()'"
using size_t = decltype(sizeof(0));
void *operator new(size_t, void *p) noexcept { return p; }
namespace std {
template <class T, class U> constexpr T *construct_at(T *p, U u) { return ::new ((void *)p) T(u); }
} // namespace std

struct E { constexpr int get() const { return 1; } };
struct F { constexpr int get() const { return 2; } };
union U {
  constexpr U() : e() {}
  [[no_unique_address]] E e;
  [[no_unique_address]] F f;
};
struct R {
  constexpr R() : u(), b(true) {}
  constexpr R(int) : u() {}
  [[no_unique_address]] U u;
  [[no_unique_address]] bool b;
};
static_assert(sizeof(R) == 1);

constexpr bool init() { R r; return r.b; }
static_assert(init());
constexpr bool assign() { R r; r.b = false; return !r.b && r.u.e.get() == 1; }
static_assert(assign());
constexpr bool construct() { R r(0); std::construct_at(&r.b, true); return r.b; }
static_assert(construct());
constexpr bool copy() { R r; R s = r; return s.b; }
static_assert(copy());

// Still diagnosed: the union's inactive member, and the flag unwritten.
constexpr int inactive_call() { R r; return r.u.f.get(); }
static_assert(inactive_call() == 2);
constexpr bool flag_uninit() { R r(0); return r.b; }
static_assert(flag_uninit());
