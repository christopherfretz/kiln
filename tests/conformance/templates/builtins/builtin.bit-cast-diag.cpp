// __builtin_bit_cast (spec/sema-templates.md §9.5, [bit.cast]/1): both
// types complete, the same size, and trivially copyable; the keyword form
// parses `(type-id, expression)`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:9 error "cast to incomplete type 'I'"
//@ diag: 20:8 note "forward declaration of 'I'"
//@ diag: 23:9 error "incomplete type 'I' where a complete type is required"
//@ diag: 20:8 note "forward declaration of 'I'"
//@ diag: 26:9 error "__builtin_bit_cast source type must be trivially copyable"
//@ diag: 27:8 error "__builtin_bit_cast destination type must be trivially copyable"
//@ diag: 28:9 error "__builtin_bit_cast source size does not equal destination size (4 vs 8)"
//@ diag: 29:31 error "expected ','"
//@ diag: 29:31 error "expected ','"
//@ diag: 30:27 error "expected '(' after '__builtin_bit_cast'"
//@ diag: 31:69 error "__builtin_bit_cast source size does not equal destination size (4 vs 8)"
//@ diag: 32:25 note "in instantiation of member function 'BC<long, int>::cast' requested here"
//@ requires: clang<22
// clang 22 emits error: '__builtin_bit_cast' source type must be trivially copyable; WP-355 residual
struct I;
extern I i;
int a = __builtin_bit_cast(I, 1);
int b = __builtin_bit_cast(int, i);
struct NT { NT(const NT &); int x; };
extern NT nt;
int c = __builtin_bit_cast(int, nt);
NT d = __builtin_bit_cast(NT, 1);
int e = __builtin_bit_cast(long, 1);
int f = __builtin_bit_cast(int);
int g = __builtin_bit_cast;
template <class T, class U> struct BC { static T cast(U u) { return __builtin_bit_cast(T, u); } };
long x = BC<long, int>::cast(1);
