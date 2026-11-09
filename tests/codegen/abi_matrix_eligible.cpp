// [special]/6, [class.temporary]/3, Itanium C++ ABI §3.1.2.3: only the
// eligible copy/move constructors and the selected destructor decide
// whether a class is passed in registers. A non-trivial member that is
// ineligible (its constraints are unsatisfied, or a more-constrained
// overload of the same signature exists) does not force the indirect
// passing, and a deleted one never does (clang `canPassInRegisters`
// skips `isDeleted() || isIneligibleOrNotSelected()`; WP-1109). Compared
// with clang 19 and clang 22 on both targets.
#include "Inputs/abi_matrix.h"

template <bool B> struct CopyK {
  long a;
  CopyK(const CopyK &) requires B = default;
  CopyK(const CopyK &) { a = 1; }
};
template <bool B> struct MoveK {
  long a, b;
  MoveK(MoveK &&) requires B = default;
  MoveK(MoveK &&) { a = 2; }
  MoveK(const MoveK &) = default;
};
template <bool B> struct DtorK {
  int a;
  ~DtorK() requires B = default;
  ~DtorK() {}
};
// A deleted copy constructor next to a defaulted move constructor.
struct DelCopy { long a; DelCopy(const DelCopy &) = delete; DelCopy(DelCopy &&) = default; };

using CopyT = CopyK<true>;
using CopyF = CopyK<false>;
using MoveT = MoveK<true>;
using MoveF = MoveK<false>;
using DtorT = DtorK<true>;
using DtorF = DtorK<false>;
ABI_SHAPE_FIXED(CopyT)
ABI_SHAPE_FIXED(CopyF)
ABI_SHAPE_FIXED(MoveT)
ABI_SHAPE_FIXED(MoveF)
ABI_SHAPE_FIXED(DtorT)
ABI_SHAPE_FIXED(DtorF)

DelCopy make_delcopy();
DelCopy pass_delcopy(DelCopy);
void use_delcopy() { pass_delcopy(make_delcopy()); }
