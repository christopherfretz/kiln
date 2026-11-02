// [meta.unary.prop] is_trivially_copyable, [class.prop]/1,
// [class.copy.ctor]/11, [class.copy.assign]/9: a member whose class declares
// only a defaulted copy constructor/assignment has no move members, so a move
// of the enclosing class selects the trivial copy; the implicit and the
// explicitly defaulted move members stay trivial (clang
// `SpecialMemberIsTrivial`). libc++ mdspan `layout_*::mapping` members.
struct M0 { M0() = default; M0(const M0&) = default; M0& operator=(const M0&) = default; int x{}; };
struct S0 { M0 m; };
static_assert(__is_trivially_copyable(S0));
static_assert(__is_trivially_constructible(S0, S0&&));
static_assert(__is_trivially_assignable(S0&, S0&&));

template <class T> struct Q {};
template <class U> struct M2 {
  M2() = default;
  M2(const M2&) = default;
  template <class V> requires(sizeof(V) > 0) explicit(sizeof(V) != 1) M2(const M2<V>&) {}
  M2& operator=(const M2&) = default;
  int x{};
};
struct S2 { M2<int> m; };
static_assert(__is_trivially_copyable(S2));

struct D1 { M0 m{}; D1(const D1&) = default; D1(D1&&) = default; };
static_assert(__is_trivially_copyable(D1));
struct D2 { M0 m{}; D2(D2&&) = default; };
static_assert(__is_trivially_constructible(D2, D2&&));
struct D3 { M0 m{}; D3& operator=(D3&&) = default; };
static_assert(__is_trivially_assignable(D3&, D3&&));

// The copy selected for a move is user-provided: not trivial.
struct NT { NT() = default; NT(const NT&) {} NT& operator=(const NT&) { return *this; } };
struct N1 { NT m; };
static_assert(!__is_trivially_copyable(N1));
struct N2 { NT m{}; N2(N2&&) = default; };
static_assert(!__is_trivially_constructible(N2, N2&&));
struct N3 { NT m{}; N3& operator=(N3&&) = default; };
static_assert(!__is_trivially_assignable(N3&, N3&&));
