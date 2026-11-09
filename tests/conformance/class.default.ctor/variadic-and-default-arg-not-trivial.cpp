// [class.default.ctor]/1, /3: a constructor callable with no arguments is
// a default constructor, `T(...)` and `T(int = 0)` included; one whose
// parameter-type-list differs from the implicit one's (variadic, or with
// parameters) is never trivial, deleted or not. The same holds for a
// variadic copy or move constructor ([class.copy.ctor]/11). A deleted
// `T() = delete` stays trivial (clang `SpecialMemberIsTrivial`; WP-1090).
//@ std: c++26
//@ expect: compile-ok
//@ args: -Wno-deprecated-builtins
//@ strict-diags
struct A { A(...); };
struct B { B(...) = delete; };
struct C { C(int = 0); };
struct D { D(int = 0) = delete; };
struct E { E() = delete; };
struct F { F() = default; };
struct G { template <class... X> G(X...); };
struct H { template <class... X> H(X...) = delete; };
struct J { J(int = 0, ...) = delete; };
struct K { K(const K &, ...) = delete; K() = default; };
struct M { M(M &&, ...) = delete; M() = default; };
struct SB { B m; };
struct DB : B {};

// The deleted variadic default constructor (clang class.union/p1.cpp s8).
static_assert(!__has_trivial_constructor(B));
static_assert(!__is_trivial(B));
static_assert(!__is_pod(B));
static_assert(!__is_literal_type(B));
static_assert(!__is_constructible(B));
static_assert(!__is_trivially_constructible(B));
static_assert(__is_trivially_copyable(B));
static_assert(!__is_aggregate(B));
static_assert(!__has_trivial_constructor(SB) && !__is_trivial(SB));
static_assert(!__has_trivial_constructor(DB) && !__is_trivial(DB));

// The rest of the matrix.
static_assert(!__has_trivial_constructor(A) && !__is_trivial(A));
static_assert(__is_constructible(A) && !__is_trivially_constructible(A));
static_assert(!__has_trivial_constructor(C) && !__is_trivial(C));
static_assert(!__has_trivial_constructor(D) && !__is_trivial(D));
static_assert(!__is_literal_type(D));
static_assert(!__has_trivial_constructor(J) && !__is_trivial(J));
static_assert(__has_trivial_constructor(E) && __is_trivial(E) && __is_pod(E));
static_assert(!__is_constructible(E) && __is_literal_type(E));
static_assert(__has_trivial_constructor(F) && __is_trivial(F));
static_assert(__is_trivially_constructible(F) && __is_nothrow_constructible(F));
static_assert(!__has_trivial_constructor(G) && !__is_trivial(G));
static_assert(!__has_trivial_constructor(H) && !__is_trivial(H));

// A deleted variadic copy or move constructor is not trivial either.
static_assert(!__is_trivially_copyable(K) && !__has_trivial_copy(K));
static_assert(!__is_trivial(K) && !__is_pod(K));
static_assert(!__is_trivially_copyable(M) && !__is_trivial(M));

// Constant evaluation: `__builtin_bit_cast` needs a trivially copyable
// type ([bit.cast]/1), so it is not available for K.
template <class T> constexpr bool bit_castable = requires { __builtin_bit_cast(T, 0); };
static_assert(!bit_castable<K>);
static_assert(bit_castable<int>);
