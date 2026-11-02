// [c.math.abs], clang -Wabsolute-value (on by default): a call of an
// absolute value function (the C library's abs/labs/llabs/fabs, their
// __builtin_ forms, or std::abs) whose argument, implicit conversions
// ignored, has an unsigned integer type has no effect; the note names the
// function (std::abs for one declared in namespace std). Signed and
// floating arguments, and calls of other functions, are not diagnosed; a
// call in a template is checked when instantiated. Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 32:9 warning "taking the absolute value of unsigned type 'unsigned char' has no effect"
//@ diag: 32:9 note "remove the call to 'abs' since unsigned values cannot be negative"
//@ diag: 33:9 warning "taking the absolute value of unsigned type 'unsigned int' has no effect"
//@ diag: 33:9 note "remove the call to 'labs' since unsigned values cannot be negative"
//@ diag: 34:9 warning "taking the absolute value of unsigned type 'unsigned long long' has no effect"
//@ diag: 34:9 note "remove the call to '__builtin_llabs' since unsigned values cannot be negative"
//@ diag: 35:9 warning "taking the absolute value of unsigned type 'bool' has no effect"
//@ diag: 35:9 note "remove the call to 'fabs' since unsigned values cannot be negative"
//@ diag: 36:9 warning "taking the absolute value of unsigned type 'E' has no effect"
//@ diag: 36:9 note "remove the call to 'abs' since unsigned values cannot be negative"
//@ diag: 37:9 warning "taking the absolute value of unsigned type 'unsigned long' has no effect"
//@ diag: 37:9 note "remove the call to 'std::abs' since unsigned values cannot be negative"
//@ diag: 30:50 warning "taking the absolute value of unsigned type 'unsigned short' has no effect"
//@ diag: 42:10 note "in instantiation of function template specialization 'in_template<unsigned short>' requested here"
//@ diag: 30:50 note "remove the call to 'abs' since unsigned values cannot be negative"
extern "C" int abs(int);
extern "C" long labs(long);
extern "C" double fabs(double);
namespace std { inline namespace v1 { long abs(long x) { return x < 0 ? -x : x; } } }
enum E : unsigned { e1 = 1 };
template <class T> int in_template(T t) { return abs(t); }
int f(unsigned char uc, unsigned ui, unsigned long long ull, unsigned long ul, int i, double d) {
  (void)abs(uc);
  (void)labs(ui);
  (void)__builtin_llabs(ull);
  (void)fabs(true);
  (void)abs(e1);
  (void)std::abs(ul);
  (void)abs(i);
  (void)fabs(d);
  (void)std::abs(i);
  (void)__builtin_abs(-3);
  return in_template<unsigned short>(1) + in_template(2);
}
