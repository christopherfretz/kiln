struct __attribute__((aligned(16))) A { char c; };
struct __attribute__((aligned)) B { char c; };
struct C { char c; int i __attribute__((aligned(8))); };
typedef int Int8 __attribute__((aligned(8)));
struct D { char c; Int8 i; };
