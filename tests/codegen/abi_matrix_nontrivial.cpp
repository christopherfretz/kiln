// [class.temporary]/3, Itanium C++ ABI §3.1.2.3: a class that is not
// trivially copyable for calls is passed by address of a caller-made
// temporary and returned via `sret`; `[[clang::trivial_abi]]` restores
// register passing (spec/landed/abi/WP-766.md). Compared with clang 19
// and clang 22 on both targets.
#include "Inputs/abi_matrix.h"
struct NCopy { int a; NCopy(const NCopy &); NCopy(int); };
struct NDtor { int a; ~NDtor(); };
struct NMove { long a, b; NMove(NMove &&); NMove(const NMove &) = default; };
struct NDel { int a; NDel(const NDel &) = delete; NDel(NDel &&) = delete; };
struct NBig { long a[5]; ~NBig(); };
struct NEmpty { NEmpty(const NEmpty &); NEmpty(); };
struct NHfa { float a, b; ~NHfa(); };
struct NVirt { virtual void f(); int a; };
struct NMem { NDtor m; };
struct NBase : NCopy { using NCopy::NCopy; };
struct [[clang::trivial_abi]] TA { int *p; ~TA(); };
struct [[clang::trivial_abi]] TAHfa { double a, b; TAHfa(const TAHfa &); ~TAHfa(); };
struct [[clang::trivial_abi]] TABig { long a[3]; ~TABig(); };
struct DefCopy { int a; DefCopy(const DefCopy &) = default; DefCopy(int); };
ABI_SHAPE_FIXED(NCopy)
ABI_SHAPE_FIXED(NDtor)
ABI_SHAPE_FIXED(NMove)
ABI_SHAPE_FIXED(NBig)
ABI_SHAPE_FIXED(NEmpty)
ABI_SHAPE_FIXED(NHfa)
ABI_SHAPE_FIXED(NVirt)
ABI_SHAPE_FIXED(NMem)
ABI_SHAPE_FIXED(NBase)
ABI_SHAPE_FIXED(TA)
ABI_SHAPE_FIXED(TAHfa)
ABI_SHAPE_FIXED(TABig)
ABI_SHAPE_NP(DefCopy)
// A class whose copy and move constructors are deleted is still passed
// indirectly; it can only be returned by guaranteed elision.
NDel make_ndel();
NDel pass_ndel(NDel);
void use_ndel() { pass_ndel(make_ndel()); }
