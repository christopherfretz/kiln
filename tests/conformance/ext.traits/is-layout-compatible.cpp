// __is_layout_compatible(T, U) ([basic.types.general]/11, clang 19,
// WP-166): same type ignoring cv, enums with the same underlying type,
// standard-layout structs/unions with layout-compatible common fields;
// incomplete operands (other than void and T[]) are errors.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-defaulted-function-deleted
//@ diag: 80:34 error "incomplete type 'Inc' where a complete type is required"
//@ diag: 50:8 note "forward declaration of 'Inc'"
//@ diag: 81:39 error "incomplete type 'Inc[2]' where a complete type is required"
//@ diag: 50:8 note "forward declaration of 'Inc'"
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
static_assert(__is_layout_compatible(int, int));
static_assert(__is_layout_compatible(int, const int));
static_assert(!__is_layout_compatible(int, unsigned));
static_assert(__is_layout_compatible(int[2], int[2]));
static_assert(__is_layout_compatible(const int[2], int[2]));
static_assert(!__is_layout_compatible(int[2], int[3]));
static_assert(__is_layout_compatible(int[], int[]));
static_assert(__is_layout_compatible(void, void));
static_assert(__is_layout_compatible(void, const void));
static_assert(__is_layout_compatible(int&, int&));
static_assert(__is_layout_compatible(int*, int*));
static_assert(__is_layout_compatible(S1, S2));
static_assert(!__is_layout_compatible(S1, S3));
static_assert(__is_layout_compatible(SB1, SB2));
static_assert(!__is_layout_compatible(SB1, SB3));
static_assert(!__is_layout_compatible(SB1, S1));
static_assert(!__is_layout_compatible(SAl, S1));
static_assert(!__is_layout_compatible(SNU, S1));
static_assert(__is_layout_compatible(D1, S1));
static_assert(__is_layout_compatible(U1, U2));
static_assert(!__is_layout_compatible(U1, S1));
static_assert(__is_layout_compatible(EC1, EC2));
static_assert(!__is_layout_compatible(EC1, EC3));
static_assert(!__is_layout_compatible(EC1, int));
static_assert(!__is_layout_compatible(NSL, NSL2));
static_assert(__is_layout_compatible(E, B0));
static_assert(__is_layout_compatible(Dv, S1));
static_assert(__is_layout_compatible(int(), int()));
static_assert(__is_layout_compatible(int A::*, int A::*));
bool f1 = __is_layout_compatible(Inc, int);
bool f2 = __is_layout_compatible(int, Inc[2]);
bool f3 = __is_layout_compatible(Inc[], Inc[]);
