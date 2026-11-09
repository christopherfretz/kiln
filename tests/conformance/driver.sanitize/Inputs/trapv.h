// The arithmetic the `-ftrapv` IR tests check (WP-1032).
typedef int myint;
using u16 = unsigned short;
int add(int a, int b) { return a + b; }
int sub(int a, int b) { return a - b; }
int mul(int a, int b) { return a * b; }
int neg(int a) { return -a; }
int dv(int a, int b) { return a / b; }
int shl(int a, int b) { return a << b; }
long long addl(long long a, long long b) { return a + b; }
int c1(char a, char b) { return a + b; }
unsigned c2(u16 a, u16 b) { return a * b; }
int c3(unsigned char a, unsigned char b) { return a * b; }
myint t1(myint a, myint b) { return a * b; }
void ca(int &x, int y) { x -= y; x *= 2; x += 1; }
void cs(short &x, short y) { x += y; ++x; x--; }
int pre(int x) { return ++x + x-- + --x; }
unsigned upre(unsigned x) { return x++ + ++x; }
unsigned u1(unsigned a, unsigned b) { return -a + a * b; }
int cst() { return 2147483647 + 0; }
int nc() { return -(-2147483647); }
int at(_Atomic(int) &x) { x += 3; return ++x; }
unsigned uat(_Atomic(unsigned) &x) { return x++; }
