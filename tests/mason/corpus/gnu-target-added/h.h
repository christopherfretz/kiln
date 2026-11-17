#if defined(__x86_64__)
#define TGT "avx2"
#else
#define TGT "arch=armv8.2-a"
#endif
ATTR inline int f(int a) { return a * 3; }
