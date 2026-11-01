struct S3 { char c[3]; };
struct S16 { long a, b; };
struct S32 { long a[4]; };
struct A { char c; _Atomic(char) ac; _Atomic(short) as; _Atomic(int) ai; _Atomic(long long) al; };
struct B { char c; _Atomic(S3) s3; char d; _Atomic(S16) s16; char e; _Atomic(S32) s32; };
struct C { char c; _Atomic(double) d; _Atomic(int *) p; _Atomic(bool) b; _Atomic int q; };
struct D { char c; _Atomic(__int128) w; _Atomic(long double) ld; _Atomic(float) f[3]; };
