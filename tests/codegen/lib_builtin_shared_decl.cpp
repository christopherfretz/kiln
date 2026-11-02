// [dcl.link]/6, clang GetOrCreateLLVMFunction: a library builtin called as
// `__builtin_pow` and the C library's `pow` declared by the program are one
// symbol, declared once (`pow`, never a uniqued `pow.1`), in either order.
extern "C" double pow(double, double);
extern "C" float powf(float, float);
double f(double x, double y) { return __builtin_pow(x, y); }
double g(double x, double y) { return pow(x, y); }
float h(float x, float y) { return powf(x, y); }
float k(float x, float y) { return __builtin_powf(x, y); }
