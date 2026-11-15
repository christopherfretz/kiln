// A constant __builtin_offsetof index of unsigned type (unsigned char,
// unsigned short, unsigned, bool, an unsigned enumeration, unsigned
// __int128) with the top bit of its own width set counts at its true
// value, zero-extended, as GCC does: the constant equals what the
// run-time lowering computes for the same index value and type, in a
// constexpr function called at compile time too (found building an
// open-source RPC library).
//@ skip(clang): clang 19 and 22 sign-extend an unsigned constant offsetof index from its own width
//@ expect: run
//@ stdout: 204 204 5 5 65004 65004 3000000004 3000000004
//@ stdout: 204 204 204 204 254 254
extern "C" int printf(const char*, ...);
typedef __SIZE_TYPE__ size_t;
struct C { int n; char a[300]; };
struct S { int n; char a[70000]; };
struct Big { int n; char a[4000000000u]; };
enum E : unsigned char { e200 = 200 };

constexpr size_t cx(unsigned char i) { return __builtin_offsetof(C, a[i]); }

static_assert(__builtin_offsetof(C, a[(unsigned char)200]) == 204);
static_assert(__builtin_offsetof(C, a[true]) == 5);
static_assert(__builtin_offsetof(S, a[(unsigned short)65000]) == 65004);
static_assert(__builtin_offsetof(Big, a[3000000000u]) == 3000000004u);
static_assert(cx(200) == 204);
static_assert(__builtin_offsetof(C, a[e200]) == 204);
static_assert(__builtin_offsetof(C, a[(unsigned __int128)250]) == 254);

size_t rc(unsigned char i) { return __builtin_offsetof(C, a[i]); }
size_t rb(bool b) { return __builtin_offsetof(C, a[b]); }
size_t rs(unsigned short i) { return __builtin_offsetof(S, a[i]); }
size_t ru(unsigned i) { return __builtin_offsetof(Big, a[i]); }
size_t re(E e) { return __builtin_offsetof(C, a[e]); }
size_t rq(unsigned __int128 i) { return __builtin_offsetof(C, a[i]); }

int main() {
  volatile unsigned char c = 200;
  volatile bool b = true;
  volatile unsigned short s = 65000;
  volatile unsigned u = 3000000000u;
  volatile E e = e200;
  volatile unsigned q = 250;
  constexpr size_t k = cx(200);
  printf("%zu %zu %zu %zu %zu %zu %zu %zu\n",
         __builtin_offsetof(C, a[(unsigned char)200]), rc(c),
         __builtin_offsetof(C, a[true]), rb(b),
         __builtin_offsetof(S, a[(unsigned short)65000]), rs(s),
         __builtin_offsetof(Big, a[3000000000u]), ru(u));
  printf("%zu %zu %zu %zu %zu %zu\n", k, cx(c),
         __builtin_offsetof(C, a[e200]), re(e),
         __builtin_offsetof(C, a[(unsigned __int128)250]),
         rq((unsigned __int128)q));
}
