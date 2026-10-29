typedef double D32 __attribute__((aligned(32)));
typedef int I2 __attribute__((aligned(2)));
struct S { char c; D32 d; };
struct T { char c; I2 i; };
