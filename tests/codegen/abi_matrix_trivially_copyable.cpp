// [class.temporary]/3, Itanium C++ ABI §3.1.2.3: a class is passed and
// returned in registers unless it has a non-trivial eligible copy or move
// constructor or destructor, or all its copy and move constructors are
// deleted. That is not [class.prop]/1 "trivially copyable": the
// assignment operators do not count, and a class whose copy and move
// constructors are all deleted is passed indirectly whatever its
// assignment operators are. The classes of
// tests/conformance/class.prop/trivially-copyable-matrix*.cpp, by value,
// compared with clang 19 and clang 22 on both targets (WP-1452).
#include "Inputs/abi_matrix.h"

// The prvalue positions, for a class that cannot be copied or moved.
#define ABI_SHAPE_PRVALUE(T)                                                  \
  T T##_mk();                                                                 \
  void T##_take(T);                                                           \
  T T##_ret() { return T##_mk(); }                                            \
  void T##_def(T) {}                                                          \
  void T##_use() { T##_take(T##_mk()); }

struct I { long a; };
struct E {};
struct VI { volatile long a; };
struct T1 { const long a; };
struct C1 { const E e; long a; };
struct R { long &r; };
struct RR { long &&r; };
union U1 { long a; double d; };
struct NS { long a = 0; };
struct Mut { mutable I i; };
struct CVM { const I i; };
struct DB : T1 {};
union UC { const long a; };
struct N { long a; N(...) = delete; N() = default; };
struct DCopyDefMove {
  long a;
  DCopyDefMove(const DCopyDefMove &) = delete;
  DCopyDefMove(DCopyDefMove &&) = default;
};
template <bool B> struct K2 {
  long a;
  K2(const K2 &) requires B = delete;
  K2(const K2 &) = default;
  K2(K2 &&) = delete;
  K2 &operator=(const K2 &) = delete;
};
template <bool B> struct K3 { long a; K3() requires B = delete; K3() = default; };
using K2F = K2<false>;
using K3T = K3<true>;
using K3F = K3<false>;

ABI_SHAPE_FIXED(I)
ABI_SHAPE_FIXED(VI)
ABI_SHAPE_FIXED(T1)
ABI_SHAPE_FIXED(C1)
ABI_SHAPE_FIXED(R)
ABI_SHAPE_FIXED(U1)
ABI_SHAPE_FIXED(NS)
ABI_SHAPE_FIXED(Mut)
ABI_SHAPE_FIXED(CVM)
ABI_SHAPE_FIXED(DB)
ABI_SHAPE_FIXED(UC)
ABI_SHAPE_FIXED(N)
ABI_SHAPE_FIXED(K3T)
ABI_SHAPE_FIXED(K3F)
ABI_SHAPE_PRVALUE(K2F)
ABI_SHAPE_PRVALUE(RR)
ABI_SHAPE_PRVALUE(DCopyDefMove)

// No eligible copy or move constructor (each is deleted, or ineligible as
// `K2<true>`'s unconstrained one): passed indirectly. All of these but
// `OMA` and `U2` are also not trivially copyable.
struct C5 { volatile I i; };
struct CV { const volatile I i; };
struct Arr { C5 a[2]; };
struct AllDel {
  long a;
  AllDel(const AllDel &) = delete;
  AllDel &operator=(const AllDel &) = delete;
};
struct AllDel4 {
  long a;
  AllDel4(const AllDel4 &) = delete;
  AllDel4(AllDel4 &&) = delete;
  AllDel4 &operator=(const AllDel4 &) = delete;
  AllDel4 &operator=(AllDel4 &&) = delete;
};
struct B9 : AllDel4 {};
struct M9 { AllDel4 m; };
struct OnlyMoveDel { long a; OnlyMoveDel(OnlyMoveDel &&) = delete; };
// Trivially copyable through its (trivial, eligible) move assignment.
struct OMA { long a; OMA &operator=(OMA &&) = default; };
// A variant member with a non-trivial copy constructor deletes the
// union's copy and move constructors; its assignments stay trivial.
struct NC { NC(const NC &); long a; };
union U2 { NC n; };
struct NCA { NCA(const NCA &); NCA &operator=(const NCA &); long a; };
union U3 { NCA n; };
using K2T = K2<true>;

ABI_SHAPE_PRVALUE(C5)
ABI_SHAPE_PRVALUE(CV)
ABI_SHAPE_PRVALUE(Arr)
ABI_SHAPE_PRVALUE(AllDel)
ABI_SHAPE_PRVALUE(AllDel4)
ABI_SHAPE_PRVALUE(B9)
ABI_SHAPE_PRVALUE(M9)
ABI_SHAPE_PRVALUE(OnlyMoveDel)
ABI_SHAPE_PRVALUE(OMA)
ABI_SHAPE_PRVALUE(U2)
ABI_SHAPE_PRVALUE(U3)
ABI_SHAPE_PRVALUE(K2T)
