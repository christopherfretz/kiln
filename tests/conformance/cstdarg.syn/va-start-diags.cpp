// [cstdarg.syn]/1, C 7.16.1.4 (clang `BuiltinVAStart`): `va_start` takes
// the `va_list` (bound to `__builtin_va_list &`) and the last named
// parameter; it is an error outside a variadic function (a lambda's call
// operator included) and a `-Wvarargs` warning when the second operand is
// not the last named parameter or names one of reference or promotable
// type (undefined behavior). In a template the check runs at
// instantiation. `va_end` / `va_copy` are ordinary calls. Constant
// evaluation rejects every `va_list` operation.
//@ expect: compile-fail
//@ diag: 37:39 error "'va_start' used in function with fixed args"
//@ diag: 38:74 warning "second argument to 'va_start' is not the last named parameter"
//@ diag: 39:65 error "too few arguments to function call, expected 2, have 1"
//@ diag: 39:94 error "too many arguments to function call, expected 2, have 3"
//@ diag: 40:60 warning "second argument to 'va_start' is not the last named parameter"
//@ diag: 41:68 warning "passing an object that undergoes default argument promotion to 'va_start' has undefined behavior"
//@ diag: 41:13 note "parameter of type 'char' is declared here"
//@ diag: 42:68 warning "passing an object of reference type to 'va_start' has undefined behavior"
//@ diag: 42:13 note "parameter of type 'int &' is declared here"
//@ diag: 43:48 error "non-const lvalue reference to type '__builtin_va_list' cannot bind to a value of unrelated type 'int'"
//@ diag: 43:72 error "non-const lvalue reference to type '__builtin_va_list' cannot bind to a value of unrelated type 'int'"
//@ diag: 44:65 error "'va_start' used in function with fixed args"
//@ diag: 46:83 warning "passing an object that undergoes default argument promotion to 'va_start' has undefined behavior"
//@ diag: 48:15 note "in instantiation of function template specialization 't<short>' requested here"
//@ diag: 46:28 note "parameter of type 'short' is declared here"
//@ diag: 50:15 error "constexpr variable 'kv' must be initialized by a constant expression"
//@ diag: 49:53 note "subexpression not valid in a constant expression"
//@ diag: 50:20 note "in call to 'k(1)'"
//@ diag: 53:66 warning "passing an object that undergoes default argument promotion to 'va_start' has undefined behavior"
//@ diag: 53:11 note "parameter of type 'E' is declared here"
//@ diag: 55:70 warning "passing an object that undergoes default argument promotion to 'va_start' has undefined behavior"
//@ diag: 55:15 note "parameter of type 'float' is declared here"
//@ diag: 57:68 warning "second argument to 'va_start' is not the last named parameter"
//@ diag: 59:11 error "'va_start' cannot be used outside a function"
//@ diag: 61:66 error "too many arguments to function call, expected 1, have 2"
//@ diag: 61:91 error "too few arguments to function call, expected 2, have 1"
//@ diag: 61:111 error "non-const lvalue reference to type '__builtin_va_list' cannot bind to a value of unrelated type 'int'"
void a(int n) { __builtin_va_list ap; __builtin_va_start(ap, n); }
void b(int n, int m, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); __builtin_va_end(ap); }
void c(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap); __builtin_va_start(ap, n, n); }
void d(...) { __builtin_va_list ap; __builtin_va_start(ap, 0); }
void e(char c, ...) { __builtin_va_list ap; __builtin_va_start(ap, c); }
void f(int &r, ...) { __builtin_va_list ap; __builtin_va_start(ap, r); }
void g(int n, ...) { int x; __builtin_va_start(x, n); __builtin_va_end(x); }
void h(int n, ...) { auto l = [](int k) { __builtin_va_list ap; __builtin_va_start(ap, k); }; }
struct S { void m(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); __builtin_va_end(ap); } };
template<class T> void t(T n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); __builtin_va_end(ap); }
template void t<int>(int, ...);
template void t<short>(short, ...);
constexpr int k(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); __builtin_va_end(ap); return n; }
constexpr int kv = k(1, 2);
enum E { e0 };
enum class SE { s0 };
void en(E e, ...) { __builtin_va_list ap; __builtin_va_start(ap, e); }
void sen(SE e, ...) { __builtin_va_list ap; __builtin_va_start(ap, e); }
void fl(float x, ...) { __builtin_va_list ap; __builtin_va_start(ap, x); }
void db(double x, ...) { __builtin_va_list ap; __builtin_va_start(ap, (x)); }
void lp(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n + 0); }
__builtin_va_list gap;
int gi = (__builtin_va_start(gap, 0), 0);
void vl() { auto l = [](int k, ...) { __builtin_va_list ap; __builtin_va_start(ap, k); __builtin_va_end(ap); }; }
void ve(int n, ...) { __builtin_va_list ap; __builtin_va_end(ap, ap); __builtin_va_copy(ap); __builtin_va_end(n); }
