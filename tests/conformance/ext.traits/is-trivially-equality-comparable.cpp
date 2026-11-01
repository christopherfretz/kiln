// __is_trivially_equality_comparable(T) (clang 19, WP-166): unique object
// representations (without the trivially-copyable requirement) and, for a
// class, a defaulted const T& == const T& on it and every subobject.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:25 warning "explicitly defaulted equality comparison operator is implicitly deleted"
//@ diag: 19:17 note "defaulted 'operator==' is implicitly deleted because class 'R' has a reference member"
//@ diag: 19:54 note "replace 'default' with 'delete'"
//@ diag: 76:11 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 51:8 note "forward declaration of 'Inc'"
//@ diag: 77:11 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 51:8 note "forward declaration of 'Inc'"
struct E {};
struct A { int x; bool operator==(const A&) const = default; };
struct AV { int x; bool operator==(A) const; };
struct ND { int x; bool operator==(const ND&) const; };
struct F { float f; bool operator==(const F&) const = default; };
struct P { char c; int i; bool operator==(const P&) const = default; };
struct R { int &r; bool operator==(const R&) const = default; };
struct NoEq { int x; };
struct FE { int x; friend bool operator==(const FE&, const FE&) = default; };
struct BV { int x; friend bool operator==(BV, BV) = default; };
struct Der : A { int y; bool operator==(const Der&) const = default; };
struct Arr { int a[3]; bool operator==(const Arr&) const = default; };
enum En { e0 };
union U { int i; };
struct Dtor { ~Dtor(); };
struct CopyNT { CopyNT(const CopyNT&); };
struct MoveDel { MoveDel(MoveDel&&) = delete; MoveDel(const MoveDel&) = default; };
struct AllDel { AllDel(const AllDel&) = delete; AllDel(AllDel&&) = delete; };
struct MoveNT { MoveNT(MoveNT&&); MoveNT(const MoveNT&) = default; };
struct Virt { virtual void f(); };
struct HasMNT { MoveNT m; };
struct HasDtor { Dtor d; };
struct S1 { int x; };
struct S2 { int y; };
struct S3 { int x; char c; };
struct SB1 { int x : 3; };
struct SB2 { int y : 3; };
struct SB3 { int y : 4; };
struct SAl { alignas(8) int x; };
struct SNU { [[no_unique_address]] int x; };
struct D1 : E { int x; };
union U1 { int i; char c; };
union U2 { char d; int j; };
enum class EC1 : int {}; enum class EC2 : int {}; enum class EC3 : long {};
struct NSL { int a; private: int b; };
struct NSL2 { int a; private: int b; };
struct B0 {}; struct Dv : B0 { int x; }; struct DvNSL : B0 { B0 b; };
struct Pr : private B0 { int x; };
struct Inc;
static_assert(__is_trivially_equality_comparable(int));
static_assert(!__is_trivially_equality_comparable(float));
static_assert(__is_trivially_equality_comparable(int*));
static_assert(!__is_trivially_equality_comparable(En));
static_assert(!__is_trivially_equality_comparable(int[2]));
static_assert(!__is_trivially_equality_comparable(E));
static_assert(__is_trivially_equality_comparable(A));
static_assert(__is_trivially_equality_comparable(const A));
static_assert(!__is_trivially_equality_comparable(AV));
static_assert(!__is_trivially_equality_comparable(ND));
static_assert(!__is_trivially_equality_comparable(F));
static_assert(!__is_trivially_equality_comparable(P));
static_assert(!__is_trivially_equality_comparable(R));
static_assert(!__is_trivially_equality_comparable(NoEq));
static_assert(__is_trivially_equality_comparable(FE));
static_assert(__is_trivially_equality_comparable(BV));
static_assert(__is_trivially_equality_comparable(Der));
static_assert(__is_trivially_equality_comparable(Arr));
static_assert(!__is_trivially_equality_comparable(U));
static_assert(!__is_trivially_equality_comparable(int&));
static_assert(!__is_trivially_equality_comparable(void));
static_assert(__is_trivially_equality_comparable(bool));
static_assert(!__is_trivially_equality_comparable(decltype(nullptr)));
static_assert(__is_trivially_equality_comparable(int A::*));
bool b1 = __is_trivially_equality_comparable(Inc);
bool b2 = __is_trivially_equality_comparable(Inc[2]);
template <class T> constexpr bool tec = __is_trivially_equality_comparable(T);
static_assert(tec<A> && !tec<F> && !tec<P>);
