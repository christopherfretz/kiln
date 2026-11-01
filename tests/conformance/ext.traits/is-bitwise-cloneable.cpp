// __is_bitwise_cloneable(T) (clang 19 isBitwiseCloneableType, WP-166):
// false only for void, incomplete or dependent types; arrays use their
// element, classes their fields (polymorphic classes included).
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-defaulted-function-deleted
//@ diag: 62:11 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 49:8 note "forward declaration of 'Inc'"
//@ diag: 70:15 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 49:8 note "forward declaration of 'Inc'"
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
static_assert(__is_bitwise_cloneable(int));
static_assert(__is_bitwise_cloneable(E));
static_assert(__is_bitwise_cloneable(Dtor));
static_assert(__is_bitwise_cloneable(HasDtor));
static_assert(__is_bitwise_cloneable(Virt));
static_assert(__is_bitwise_cloneable(int&));
static_assert(!__is_bitwise_cloneable(void));
static_assert(__is_bitwise_cloneable(Dtor[3]));
static_assert(__is_bitwise_cloneable(MoveNT));
static_assert(__is_bitwise_cloneable(R));
static_assert(__is_bitwise_cloneable(volatile int));
static_assert(__is_bitwise_cloneable(int[]));
bool e1 = __is_bitwise_cloneable(Inc);
struct V { virtual void f(); int x; };
struct Ref { int& r; };
struct VB : virtual V {};
static_assert(__is_bitwise_cloneable(int&&));
static_assert(__is_bitwise_cloneable(V));
static_assert(__is_bitwise_cloneable(VB));
static_assert(__is_bitwise_cloneable(Ref));
static_assert(__is_bitwise_cloneable(Inc[]));
static_assert(__is_bitwise_cloneable(Inc*));
static_assert(__is_bitwise_cloneable(void()));
static_assert(__is_bitwise_cloneable(int[2][3]));
