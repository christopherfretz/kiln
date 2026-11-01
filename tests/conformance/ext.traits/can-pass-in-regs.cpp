// __can_pass_in_regs(T) (clang 19 canPassInRegisters, WP-166): the operand
// must be an unqualified, complete class type; true when no copy/move
// constructor or destructor is non-trivial and one copy/move constructor
// is usable.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-defaulted-function-deleted
//@ diag: 64:11 error "argument 'int' is not an unqualified class type"
//@ diag: 65:11 error "argument 'const E' is not an unqualified class type"
//@ diag: 66:11 error "argument 'E &' is not an unqualified class type"
//@ diag: 67:11 error "incomplete type 'Inc' used in type trait expression"
//@ diag: 52:8 note "forward declaration of 'Inc'"
//@ diag: 69:11 error "argument 'E[2]' is not an unqualified class type"
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
static_assert(__can_pass_in_regs(E));
static_assert(!__can_pass_in_regs(Dtor));
static_assert(!__can_pass_in_regs(CopyNT));
static_assert(__can_pass_in_regs(MoveDel));
static_assert(!__can_pass_in_regs(AllDel));
static_assert(!__can_pass_in_regs(MoveNT));
static_assert(!__can_pass_in_regs(Virt));
static_assert(!__can_pass_in_regs(HasMNT));
static_assert(!__can_pass_in_regs(HasDtor));
static_assert(__can_pass_in_regs(U));
static_assert(__can_pass_in_regs(A));
bool c1 = __can_pass_in_regs(int);
bool c2 = __can_pass_in_regs(const E);
bool c3 = __can_pass_in_regs(E&);
bool c4 = __can_pass_in_regs(Inc);
bool c5 = __can_pass_in_regs(U);
bool c6 = __can_pass_in_regs(E[2]);
