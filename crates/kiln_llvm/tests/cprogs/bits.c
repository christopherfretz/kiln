// Unsigned arithmetic, shifts, bitwise ops, narrow integer types.
unsigned char lo(unsigned x) { return x & 0xff; }
short sh(short a, unsigned char b) { return (a << 3) | (b >> 1); }
unsigned rot(unsigned x, unsigned n) { return (x << n) | (x >> (32 - n)); }
unsigned long long wide(unsigned x) { return ~(unsigned long long)x ^ (x % 7u); }
_Bool flag(int x) { return x; }
