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
