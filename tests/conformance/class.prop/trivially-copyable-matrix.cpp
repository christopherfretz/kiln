// The trivially copyable / trivial matrix (WP-1452), part 1: the answers
// the standard text, clang 19/22 and GCC 16.1 all agree on. [special]/6:
// a deleted special member, or one whose constraints are unsatisfied or
// that is less constrained than another, is not eligible. [class.prop]/1:
// trivially copyable = at least one eligible copy/move constructor or
// assignment operator, each eligible one trivial, and a trivial,
// non-deleted destructor. [class.prop]/2: trivial = trivially copyable
// with one or more eligible default constructors, all trivial.
// [depr.meta.types]/4: POD = trivial and standard-layout. The
// expression-based traits ([meta.unary.prop]: overload resolution selects
// the member) and the legacy `__has_trivial_*` (GCC's documented
// semantics) do not depend on eligibility. The classes are in
// Inputs/trivially-copyable-classes.h; the rows where an implementation
// differs from the text are in trivially-copyable-matrix-{clang,gcc,text}.cpp
// (docs/status/deviations.md). `__has_trivial_destructor` stands for the
// legacy traits: a deleted destructor is trivial for it.
//@ oracle: gcc
//@ args: -Wno-deprecated-builtins
//@ strict-diags
#include "Inputs/trivially-copyable-classes.h"

// TC: __is_trivially_copyable(T) ([class.prop]/1)
static_assert(TC(I)); static_assert(TC(E)); static_assert(TC(VI));
static_assert(TC(T1)); static_assert(TC(R)); static_assert(TC(RR));
static_assert(TC(U1)); static_assert(!TC(U3)); static_assert(TC(NS));
static_assert(TC(DefDel)); static_assert(TC(K2<false>));
static_assert(TC(K3<true>)); static_assert(TC(K3<false>));
static_assert(!TC(V)); static_assert(!TC(UP)); static_assert(TC(OMA));
static_assert(TC(PD)); static_assert(TC(Mut)); static_assert(TC(DB));
static_assert(TC(UC)); static_assert(TC(N)); static_assert(TC(NDV));
static_assert(TC(DCopyDefMove));

// TRIV: __is_trivial(T) ([class.prop]/2)
static_assert(TRIV(I)); static_assert(TRIV(E)); static_assert(!TRIV(CV));
static_assert(TRIV(VI)); static_assert(!TRIV(AllDel)); static_assert(TRIV(U1));
static_assert(!TRIV(U3)); static_assert(!TRIV(NS));
static_assert(!TRIV(K2<true>)); static_assert(!TRIV(K2<false>));
static_assert(TRIV(K3<false>)); static_assert(!TRIV(V));
static_assert(!TRIV(UP)); static_assert(TRIV(OMA)); static_assert(TRIV(PD));
static_assert(TRIV(Mut)); static_assert(!TRIV(CVM));
static_assert(!TRIV(OnlyMoveDel)); static_assert(!TRIV(NDV));
static_assert(TRIV(DCopyDefMove)); static_assert(!TRIV(DefDelCopy));
static_assert(!TRIV(VarDel));

// POD: __is_pod(T) ([depr.meta.types]/4)
static_assert(POD(I)); static_assert(POD(E)); static_assert(!POD(CV));
static_assert(POD(VI)); static_assert(!POD(R)); static_assert(!POD(RR));
static_assert(!POD(AllDel)); static_assert(POD(U1)); static_assert(!POD(U3));
static_assert(!POD(NS)); static_assert(!POD(K2<true>));
static_assert(!POD(K2<false>)); static_assert(POD(K3<false>));
static_assert(!POD(V)); static_assert(!POD(UP)); static_assert(POD(OMA));
static_assert(POD(PD)); static_assert(POD(Mut)); static_assert(!POD(CVM));
static_assert(!POD(OnlyMoveDel)); static_assert(!POD(NDV));
static_assert(POD(DCopyDefMove)); static_assert(!POD(DefDelCopy));
static_assert(!POD(VarDel));

// HTD: __has_trivial_destructor(T) (legacy)
static_assert(HTD(I)); static_assert(HTD(E)); static_assert(HTD(C5));
static_assert(HTD(CV)); static_assert(HTD(VI)); static_assert(HTD(T1));
static_assert(HTD(C1)); static_assert(HTD(R)); static_assert(HTD(RR));
static_assert(HTD(AllDel)); static_assert(HTD(AllDel4));
static_assert(HTD(DelDtor)); static_assert(HTD(U1)); static_assert(HTD(U2));
static_assert(HTD(U3)); static_assert(HTD(NS)); static_assert(HTD(DefDel));
static_assert(HTD(K2<true>)); static_assert(HTD(K2<false>));
static_assert(HTD(K3<true>)); static_assert(HTD(K3<false>));
static_assert(HTD(B9)); static_assert(HTD(M9)); static_assert(HTD(Arr));
static_assert(HTD(V)); static_assert(HTD(UP)); static_assert(HTD(OMA));
static_assert(HTD(PD)); static_assert(HTD(Mut)); static_assert(HTD(CVM));
static_assert(HTD(OnlyMoveDel)); static_assert(HTD(DB));
static_assert(HTD(UC)); static_assert(HTD(N)); static_assert(HTD(NDV));
static_assert(HTD(DCopyDefMove)); static_assert(HTD(Arr1));
static_assert(HTD(VArr)); static_assert(HTD(DefDelCopy));
static_assert(HTD(VarDel)); static_assert(HTD(CB)); static_assert(HTD(UV));

// TDC: __is_trivially_constructible(T) ([meta.unary.prop])
static_assert(TDC(I)); static_assert(TDC(E)); static_assert(TDC(C5));
static_assert(!TDC(CV)); static_assert(TDC(VI)); static_assert(!TDC(T1));
static_assert(TDC(C1)); static_assert(!TDC(R)); static_assert(!TDC(RR));
static_assert(!TDC(AllDel)); static_assert(TDC(AllDel4));
static_assert(!TDC(DelDtor)); static_assert(TDC(U1)); static_assert(!TDC(NS));
static_assert(!TDC(DefDel)); static_assert(!TDC(K2<true>));
static_assert(!TDC(K2<false>)); static_assert(TDC(K3<false>));
static_assert(TDC(B9)); static_assert(TDC(M9)); static_assert(TDC(Arr));
static_assert(!TDC(V)); static_assert(!TDC(UP)); static_assert(TDC(OMA));
static_assert(!TDC(PD)); static_assert(TDC(Mut)); static_assert(!TDC(CVM));
static_assert(!TDC(OnlyMoveDel)); static_assert(!TDC(DB));
static_assert(!TDC(N)); static_assert(!TDC(NDV));
static_assert(TDC(DCopyDefMove)); static_assert(TDC(Arr1));
static_assert(TDC(VArr)); static_assert(!TDC(DefDelCopy));
static_assert(!TDC(VarDel)); static_assert(TDC(CB)); static_assert(TDC(UV));

// TCC: __is_trivially_constructible(T, const T&) ([meta.unary.prop])
static_assert(TCC(I)); static_assert(TCC(E)); static_assert(!TCC(C5));
static_assert(!TCC(CV)); static_assert(TCC(VI)); static_assert(TCC(T1));
static_assert(TCC(C1)); static_assert(TCC(R)); static_assert(!TCC(RR));
static_assert(!TCC(AllDel)); static_assert(!TCC(AllDel4));
static_assert(!TCC(DelDtor)); static_assert(TCC(U1)); static_assert(!TCC(U2));
static_assert(!TCC(U3)); static_assert(TCC(NS)); static_assert(TCC(DefDel));
static_assert(TCC(K2<false>)); static_assert(TCC(K3<true>));
static_assert(TCC(K3<false>)); static_assert(!TCC(B9));
static_assert(!TCC(M9)); static_assert(!TCC(Arr)); static_assert(!TCC(V));
static_assert(!TCC(UP)); static_assert(!TCC(OMA)); static_assert(!TCC(PD));
static_assert(TCC(Mut)); static_assert(TCC(CVM));
static_assert(!TCC(OnlyMoveDel)); static_assert(TCC(DB));
static_assert(TCC(UC)); static_assert(TCC(N)); static_assert(TCC(NDV));
static_assert(!TCC(DCopyDefMove)); static_assert(!TCC(Arr1));
static_assert(!TCC(VArr)); static_assert(!TCC(DefDelCopy));
static_assert(!TCC(VarDel)); static_assert(!TCC(CB)); static_assert(!TCC(UV));

// TCA: __is_trivially_assignable(T&, const T&) ([meta.unary.prop])
static_assert(TCA(I)); static_assert(TCA(E)); static_assert(!TCA(C5));
static_assert(!TCA(CV)); static_assert(TCA(VI)); static_assert(!TCA(T1));
static_assert(!TCA(C1)); static_assert(!TCA(R)); static_assert(!TCA(RR));
static_assert(!TCA(AllDel)); static_assert(!TCA(AllDel4));
static_assert(TCA(DelDtor)); static_assert(TCA(U1)); static_assert(TCA(U2));
static_assert(!TCA(U3)); static_assert(TCA(NS)); static_assert(TCA(DefDel));
static_assert(!TCA(K2<true>)); static_assert(!TCA(K2<false>));
static_assert(TCA(K3<true>)); static_assert(TCA(K3<false>));
static_assert(!TCA(B9)); static_assert(!TCA(M9)); static_assert(!TCA(Arr));
static_assert(!TCA(V)); static_assert(TCA(UP)); static_assert(!TCA(OMA));
static_assert(TCA(PD)); static_assert(TCA(Mut)); static_assert(!TCA(CVM));
static_assert(!TCA(OnlyMoveDel)); static_assert(!TCA(DB));
static_assert(!TCA(UC)); static_assert(TCA(N)); static_assert(TCA(NDV));
static_assert(!TCA(DCopyDefMove)); static_assert(!TCA(Arr1));
static_assert(!TCA(VArr)); static_assert(!TCA(DefDelCopy));
static_assert(!TCA(VarDel)); static_assert(!TCA(CB)); static_assert(!TCA(UV));

// TD: __is_trivially_destructible(T) ([meta.unary.prop])
static_assert(TD(I)); static_assert(TD(E)); static_assert(TD(C5));
static_assert(TD(CV)); static_assert(TD(VI)); static_assert(TD(T1));
static_assert(TD(C1)); static_assert(TD(R)); static_assert(TD(RR));
static_assert(TD(AllDel)); static_assert(TD(AllDel4));
static_assert(!TD(DelDtor)); static_assert(TD(U1)); static_assert(TD(U2));
static_assert(TD(U3)); static_assert(TD(NS)); static_assert(TD(DefDel));
static_assert(TD(K2<true>)); static_assert(TD(K2<false>));
static_assert(TD(K3<true>)); static_assert(TD(K3<false>));
static_assert(TD(B9)); static_assert(TD(M9)); static_assert(TD(Arr));
static_assert(TD(V)); static_assert(TD(UP)); static_assert(TD(OMA));
static_assert(!TD(PD)); static_assert(TD(Mut)); static_assert(TD(CVM));
static_assert(TD(OnlyMoveDel)); static_assert(TD(DB)); static_assert(TD(UC));
static_assert(TD(N)); static_assert(TD(NDV)); static_assert(TD(DCopyDefMove));
static_assert(TD(Arr1)); static_assert(TD(VArr));
static_assert(TD(DefDelCopy)); static_assert(TD(VarDel));
static_assert(TD(CB)); static_assert(TD(UV));

// LIT: __is_literal_type(T) ([basic.types.general]/10)
static_assert(LIT(I)); static_assert(LIT(E)); static_assert(!LIT(C5));
static_assert(!LIT(CV)); static_assert(!LIT(VI)); static_assert(LIT(T1));
static_assert(LIT(C1)); static_assert(LIT(R)); static_assert(LIT(RR));
static_assert(!LIT(AllDel)); static_assert(LIT(AllDel4));
static_assert(LIT(U1)); static_assert(!LIT(U2)); static_assert(!LIT(U3));
static_assert(LIT(NS)); static_assert(LIT(DefDel));
static_assert(!LIT(K2<true>)); static_assert(!LIT(K2<false>));
static_assert(LIT(K3<true>)); static_assert(LIT(K3<false>));
static_assert(LIT(B9)); static_assert(LIT(M9)); static_assert(!LIT(Arr));
static_assert(LIT(V)); static_assert(!LIT(UP)); static_assert(LIT(OMA));
static_assert(LIT(PD)); static_assert(LIT(Mut)); static_assert(LIT(CVM));
static_assert(!LIT(OnlyMoveDel)); static_assert(LIT(DB));
static_assert(LIT(UC)); static_assert(LIT(N));
static_assert(LIT(DCopyDefMove)); static_assert(!LIT(Arr1));
static_assert(!LIT(VArr)); static_assert(!LIT(DefDelCopy));
static_assert(!LIT(VarDel)); static_assert(!LIT(CB)); static_assert(!LIT(UV));

// ILT: __builtin_is_implicit_lifetime(T) ([class.prop]/9)
#if __has_builtin(__builtin_is_implicit_lifetime)
static_assert(ILT(I)); static_assert(ILT(E)); static_assert(ILT(C5));
static_assert(ILT(CV)); static_assert(ILT(VI)); static_assert(ILT(T1));
static_assert(ILT(C1)); static_assert(ILT(R)); static_assert(ILT(RR));
static_assert(!ILT(AllDel)); static_assert(ILT(AllDel4));
static_assert(ILT(DelDtor)); static_assert(ILT(U1)); static_assert(ILT(U2));
static_assert(ILT(U3)); static_assert(ILT(NS)); static_assert(ILT(DefDel));
static_assert(ILT(K2<false>)); static_assert(ILT(K3<true>));
static_assert(ILT(K3<false>)); static_assert(ILT(B9)); static_assert(ILT(M9));
static_assert(ILT(Arr)); static_assert(!ILT(V)); static_assert(!ILT(UP));
static_assert(ILT(OMA)); static_assert(ILT(PD)); static_assert(ILT(Mut));
static_assert(ILT(CVM)); static_assert(!ILT(OnlyMoveDel));
static_assert(ILT(DB)); static_assert(ILT(UC)); static_assert(ILT(N));
static_assert(ILT(NDV)); static_assert(ILT(DCopyDefMove));
static_assert(ILT(Arr1)); static_assert(ILT(VArr));
static_assert(!ILT(DefDelCopy)); static_assert(ILT(VarDel));
static_assert(ILT(CB)); static_assert(ILT(UV));
#endif
