// WP-165: `_Atomic` aggregates initialized from and assigned to (clang
// `EmitAtomicInit`, `AggExprEmitter::VisitBinAssign` with
// `EmitAtomicStore`, and the `NonAtomicToAtomic` aggregate cast).
struct S3 { char c[3]; };
struct S8 { int a, b; };
struct S32 { long a[4]; };
struct D3 : S3 {};
_Atomic(S3) a3, b3;
_Atomic(S8) a8;
_Atomic(S32) a32, b32;
S8 v8;
void init8() { _Atomic(S8) d = v8; }
void init3(S3 v) { _Atomic(S3) e(v); _Atomic(S3) f{v}; }
void init32(S32 v) { _Atomic(S32) e{v}; _Atomic(S32) f = v; }
void initd(D3 d) { _Atomic(S3) e = d; }
void assign8() { _Atomic(S8) b; b = a8; }
void assign3() { a3 = b3; }
void assign3v(S3 v) { a3 = v; }
void assign32() { a32 = b32; }
void assign32v(S32 v) { a32 = v; }
void assignvol(volatile _Atomic(S3) &v) { v = a3; a3 = v; }
// WP-183: a `return` operand (clang's `NeedAtomicConversion`
// in `InitializationSequence::InitializeFrom`).
_Atomic(S8) ret8(S8 &s) { return s; }
_Atomic(S8) ret8v(S8 s) { return s; }
_Atomic(S3) retd(D3 d) { return d; }
_Atomic(S8) retl() { return S8{1, 2}; }
// WP-183: `_Atomic` aggregate parameters and results are passed as clang's
// `isAggregateTypeForABI` aggregates (x86-64 memory, AArch64 integers).
void par8(_Atomic(S8) p) {}
void par3(_Atomic(S3) p) {}
void par32(_Atomic(S32) p) {}
_Atomic(S8) res8();
_Atomic(S3) res3();
_Atomic(S32) res32();
void calls(_Atomic(S8) &a, _Atomic(S3) &b) {
  _Atomic(S8) x = res8();
  _Atomic(S3) y = res3();
  _Atomic(S32) z = res32();
  a = res8();
  b = res3();
  par8(res8());
  par3(res3());
  par32(res32());
}
