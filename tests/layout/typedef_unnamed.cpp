typedef struct { int a; char b; } TD;
typedef union { int a; double d; } TU;
struct Uses { TD td; TU tu; char c; };
