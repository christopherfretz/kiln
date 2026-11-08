// The arithmetic the UBSan IR tests check (WP-675).
typedef int myint;
enum E { A, B };
using u16 = unsigned short;
int add(int a, int b) { return a + b; }
int sub(int a, int b) { return a - b; }
int mul(int a, int b) { return a * b; }
int neg(int a) { return -a; }
int dv(int a, int b) { return a / b; }
int rm(int a, int b) { return a % b; }
unsigned udv(unsigned a, unsigned b) { return a / b; }
int shl(int a, int b) { return a << b; }
int shr(int a, int b) { return a >> b; }
long long addl(long long a, long long b) { return a + b; }
long sl(long a, char b) { return a << b; }
int c1(char a, char b) { return a + b; }
unsigned c2(u16 a, u16 b) { return a * b; }
int c3(unsigned char a, unsigned char b) { return a * b; }
myint t1(myint a, myint b) { return a * b; }
int e1(E a, int b) { return a + b; }
void ca(int &x, int y) { x <<= y; x /= y; x -= y; x *= 2; }
void cs(short &x, short y) { x += y; ++x; x--; }
int pre(int x) { return ++x + x-- + --x; }
unsigned upre(unsigned x) { return x++ + ++x; }
unsigned u1(unsigned a, unsigned b) { return -a + a * b; }
__int128 w(__int128 a, __int128 b) { return a + b; }
__int128 w2(__int128 a, int b) { return a << b; }
__int128 w3(__int128 a, __int128 b) { return a / b; }
long long ll(long long a, unsigned b) { return a >> b; }
int d2(int a) { return a / 2; }
int d3(int a) { return 7 / a; }
int m1(int a) { return a % -1; }
int s1(int a) { return a << 3; }
int s2(int a) { return 1 << a; }
int cst() { return 2147483647 + 0; }
int ret(int a) { if (a) return 1; }
int unr() { __builtin_unreachable(); }
void vla(int n) { int a[n]; a[0] = 1; (void)a; }
void vla2(unsigned n) { int a[n][n]; a[0][0] = 1; (void)a; }
float fl(float a, float b) { return a + b; }
