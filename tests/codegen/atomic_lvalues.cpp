// [atomics.types.generic] support: _Atomic(T) objects. Reads and writes are
// seq_cst atomic loads and stores (initialization is a plain store); ++/--
// are atomicrmw add/sub (fadd/fsub for float, pointer stride for T*);
// integer += -= &= |= ^= are atomicrmw; everything else (and every
// non-integer compound assignment) is a cmpxchg loop.
_Atomic(float) af; _Atomic(int*) ap; _Atomic(bool) ab; volatile _Atomic(long long) vl;
_Atomic(short) gs; _Atomic(int) gi; _Atomic(bool) gb; _Atomic(unsigned) gu; _Atomic(float) gf;
_Atomic(unsigned char) gc; _Atomic(double) gd; volatile _Atomic(int) vi;
float l_f() { af = 1.5f; return af; }
int *l_p(int *p) { ap = p; return ap; }
bool l_b(bool x) { ab = x; return ab; }
long long l_v() { vl = 2; return vl; }
void inc() { _Atomic(int) i = 0; i++; ++i; i--; i += 3; i -= 2; i &= 1; i |= 4; i ^= 5; }
int inc2(_Atomic(int) &i, _Atomic(int) &j) { return ++i + j++; }
float fi(_Atomic(float) &f) { f++; return ++f; }
int *pi(_Atomic(int *) &p) { p++; return ++p; }
long la(_Atomic(long) &l, int x) { l += x; l -= 1; return l *= 2; }
void as(_Atomic(int) &i, _Atomic(int) &j) { i = j; }
void loc() { _Atomic(int) x = 5; _Atomic(long) y = 6; x = 7; y = x; }
void cmp() { gs *= 2; gc += 1; gd += 1.5; gd -= 2; gs <<= 1; gi /= 3; gb = !gb; }
int vol() { vi = 1; vi += 2; ++vi; return vi; }
void mixed() { gi += 1.5; gu -= 2.5; gs += 1; gb += 1; gf *= 2; gi %= 3; gu >>= 1; }
void steps() { gs++; --gs; gu--; gf--; }
