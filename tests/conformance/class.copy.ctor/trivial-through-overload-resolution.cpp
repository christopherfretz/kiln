// [class.copy.ctor]/11, [class.copy.assign]/9: an implicit copy/move
// constructor or assignment is trivial when the member selected for each
// subobject is. `E`'s move constructor selects `C`'s trivial copy
// constructor (C has no move constructor), so it is trivial, and a class
// deriving from or containing `E` sees it as trivial too (clang
// `addedEligibleSpecialMember`). libc++ `optional<T>` triviality.
struct C {
  C() = default;
  C(C const&) = default;
  C& operator=(C const&) = default;
};
struct E { C v; };
static_assert(__is_trivially_constructible(E, E&&));
static_assert(__is_trivially_assignable(E&, E&&));

struct S : E {};
static_assert(__is_trivially_constructible(S, S&&));
static_assert(__is_trivially_assignable(S&, S&&));
static_assert(__is_trivially_copyable(S));

struct M { E e; };
static_assert(__is_trivially_constructible(M, M&&));
static_assert(__is_trivially_assignable(M&, M&&));

struct I : E { using E::E; };
static_assert(__is_trivially_constructible(I, I&&));

// An anonymous union member, as libc++'s `__optional_destruct_base`.
struct U { union { char c; C v; }; bool e; constexpr U() noexcept : c(), e(false) {} };
struct V : U { using U::U; };
static_assert(__is_trivially_constructible(V, V&&));

// A non-trivial selection stays non-trivial.
struct N { N() = default; N(N const&) {} };
struct F { N n; };
struct G : F {};
static_assert(!__is_trivially_constructible(G, G&&));
