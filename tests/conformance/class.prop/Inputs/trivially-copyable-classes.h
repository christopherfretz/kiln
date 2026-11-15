// The classes of the trivially copyable / trivial matrix (WP-1452,
// class.prop/trivially-copyable-matrix*.cpp), with what [special]/6 makes
// eligible. "del" = deleted, hence not eligible.
#ifndef TRIVIALLY_COPYABLE_CLASSES_H
#define TRIVIALLY_COPYABLE_CLASSES_H

#define TC(T) __is_trivially_copyable(T)
#define TRIV(T) __is_trivial(T)
#define POD(T) __is_pod(T)
#define HTD(T) __has_trivial_destructor(T)
#define TDC(T) __is_trivially_constructible(T)
#define TCC(T) __is_trivially_constructible(T, const T &)
#define TCA(T) __is_trivially_assignable(T &, const T &)
#define TD(T) __is_trivially_destructible(T)
#define LIT(T) __is_literal_type(T)
#define ILT(T) __builtin_is_implicit_lifetime(T)

struct I { int i; };
struct E {};
// All four copy/move members del ([class.copy.ctor]/10, [class.copy.assign]/7).
struct C5 { volatile I i; };
struct CV { const volatile I i; };
struct Arr { C5 a[2]; };
struct Arr1 { C5 a; };
struct VArr { volatile I a[2]; };
struct CB : C5 {};
union UV { volatile I i; };
// A volatile scalar member deletes nothing (CWG2094).
struct VI { volatile int i; };
// Default constructor del ([class.default.ctor]/2), assignments del.
struct T1 { const int x; };
struct DB : T1 {};
struct R { int &r; };
struct CVM { const I i; };
// Copy constructor del, move constructor trivial.
struct RR { int &&r; };
// Assignments del, constructors trivial; default constructor trivial.
struct C1 { const E e; };
// C++26 (P3074R7): an all-const union's default constructor is not del.
union UC { const int c; };
// Explicitly deleted members.
struct AllDel { AllDel(const AllDel &) = delete; AllDel &operator=(const AllDel &) = delete; };
struct AllDel4 {
  AllDel4() = default;
  AllDel4(const AllDel4 &) = delete;
  AllDel4(AllDel4 &&) = delete;
  AllDel4 &operator=(const AllDel4 &) = delete;
  AllDel4 &operator=(AllDel4 &&) = delete;
};
struct B9 : AllDel4 {};
struct M9 { AllDel4 a; };
struct OnlyMoveDel { OnlyMoveDel(OnlyMoveDel &&) = delete; };
struct DefDel { DefDel() = delete; };
struct DelDtor { ~DelDtor() = delete; };
struct DCopyDefMove {
  DCopyDefMove(const DCopyDefMove &) = delete;
  DCopyDefMove(DCopyDefMove &&) = default;
  DCopyDefMove() = default;
};
// Only the move assignment operator is eligible.
struct OMA { OMA &operator=(OMA &&) = default; };
// A defaulted copy constructor defined as deleted; the assignments too.
struct DefDelCopy { volatile I i; DefDelCopy(const DefDelCopy &) = default; };
// A deleted variadic (non-trivial) copy constructor is not eligible.
struct VarDel { VarDel(const VarDel &, ...) = delete; VarDel(VarDel &&) = default; };
struct KV { KV(const KV &, ...) = delete; KV() = default; };
// An eligible non-trivial default constructor beside the trivial one.
struct AD { AD(int = 0); AD() = default; };
// `N(...)` is a default constructor ([class.default.ctor]/1), del.
struct N { N(...) = delete; N() = default; };
struct NDV { NDV(...) = delete; };
// Unions: a variant member with a non-trivial copy constructor deletes the
// union's copy and move constructors ([class.copy.ctor]/10.1); NC's
// assignments are trivial, NCA's are not (so U3's are del too).
union U1 { int i; float f; };
struct NC { NC(const NC &); };
union U2 { NC n; };
struct NCA { NCA(const NCA &); NCA &operator=(const NCA &); };
union U3 { NCA n; };
// Constraints (P0848): the constrained member is more constrained, so the
// unconstrained one of the same signature is not eligible ([special]/6).
template <bool B> struct K2 {
  K2(const K2 &) requires B = delete;
  K2(const K2 &) = default;
  K2(K2 &&) = delete;
  K2 &operator=(const K2 &) = delete;
};
template <bool B> struct K3 { K3() requires B = delete; K3() = default; };
// Non-trivial members.
struct NS { int x = 0; };
struct V { virtual void f(); };
struct UP { UP(const UP &); };
struct PD { private: ~PD() = default; };
struct Mut { mutable I i; };

#endif
