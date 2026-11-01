// __is_pointer_interconvertible_base_of(B, D) ([meta.rel]/5, clang 19,
// WP-166): the same non-union class, or B is a base of the complete
// standard-layout class D.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-defaulted-function-deleted
//@ diag: 59:52 error "incomplete type 'Inc' where a complete type is required"
//@ diag: 47:8 note "forward declaration of 'Inc'"
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
static_assert(__is_pointer_interconvertible_base_of(B0, Dv));
static_assert(!__is_pointer_interconvertible_base_of(B0, DvNSL));
static_assert(__is_pointer_interconvertible_base_of(B0, Pr));
static_assert(!__is_pointer_interconvertible_base_of(Dv, B0));
static_assert(__is_pointer_interconvertible_base_of(B0, B0));
static_assert(__is_pointer_interconvertible_base_of(const B0, B0));
static_assert(__is_pointer_interconvertible_base_of(Inc, Inc));
static_assert(!__is_pointer_interconvertible_base_of(E, A));
static_assert(!__is_pointer_interconvertible_base_of(int, int));
static_assert(!__is_pointer_interconvertible_base_of(U, U));
static_assert(!__is_pointer_interconvertible_base_of(B0&, Dv));
bool g1 = __is_pointer_interconvertible_base_of(E, Inc);
bool g2 = __is_pointer_interconvertible_base_of(Inc, E);
bool g3 = __is_pointer_interconvertible_base_of(Inc, Inc);
bool g4 = __is_pointer_interconvertible_base_of(U, Inc);
