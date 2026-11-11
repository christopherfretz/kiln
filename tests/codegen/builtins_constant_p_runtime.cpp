// WP-1288: `__builtin_constant_p` of a value the frontend cannot fold is
// left to the optimizer as `zext(llvm.is.constant.<ty>(x))` when the
// operand is an integral, enumeration or real floating value without
// possible side effects, and is 0 (the operand not evaluated) for a
// pointer, a class, an array, `x++` or a call (clang CGBuiltin
// BI__builtin_constant_p).

int global;
enum E { e0, e1 };
struct S { int a; };
int side();

int f_int(int x) { return __builtin_constant_p(x); }
int f_ptr(const char *x) { return __builtin_constant_p(x); }
int f_global() { return __builtin_constant_p(global); }
int f_inc(int x) { return __builtin_constant_p(x++); }
int f_double(double d) { return __builtin_constant_p(d); }
int f_float(float d) { return __builtin_constant_p(d + 1.0f); }
int f_class(S s) { return __builtin_constant_p(s); }
int f_deref(int *p) { return __builtin_constant_p(*p); }
int f_array() { char buf[10]; return __builtin_constant_p(buf); }
int f_addr() { return __builtin_constant_p(&global); }
int f_ll(long long x) { return __builtin_constant_p(x + 1); }
int f_bool(bool b) { return __builtin_constant_p(b); }
int f_enum(E e) { return __builtin_constant_p(e); }
int f_char(char c) { return __builtin_constant_p(c); }
int f_call() { return __builtin_constant_p(side()); }
int f_assign(int x) { return __builtin_constant_p(x = 2); }
