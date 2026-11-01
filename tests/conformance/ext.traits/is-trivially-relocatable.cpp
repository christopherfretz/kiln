// __is_trivially_relocatable(T) (clang 19, WP-166): a complete non-class
// object type, or a class that can be passed in registers.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-defaulted-function-deleted
//@ diag: 64:11 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 48:8 note "forward declaration of 'Inc'"
//@ diag: 65:11 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 48:8 note "forward declaration of 'Inc'"
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
static_assert(__is_trivially_relocatable(int));
static_assert(!__is_trivially_relocatable(int&));
static_assert(__is_trivially_relocatable(int[]));
static_assert(__is_trivially_relocatable(int[3]));
static_assert(__is_trivially_relocatable(E));
static_assert(!__is_trivially_relocatable(Dtor));
static_assert(!__is_trivially_relocatable(MoveNT));
static_assert(__is_trivially_relocatable(MoveDel));
static_assert(!__is_trivially_relocatable(AllDel));
static_assert(!__is_trivially_relocatable(Virt));
static_assert(!__is_trivially_relocatable(HasDtor[2]));
static_assert(__is_trivially_relocatable(const E));
static_assert(__is_trivially_relocatable(volatile int));
static_assert(!__is_trivially_relocatable(void));
static_assert(!__is_trivially_relocatable(void()));
bool d1 = __is_trivially_relocatable(Inc);
bool d2 = __is_trivially_relocatable(Inc[]);
