// [conv.integral]/3 (WP-217): -Wconstant-conversion (on by default; clang
// warn_impcast_integer_precision_constant): an integral constant
// implicitly converted to a narrower integer type whose value changes:
// the value needs more bits than the target, or a non-negative signed
// value of exactly the target's width becomes negative (not for a
// hexadecimal, octal or binary literal). The arms of a conditional are
// checked separately; the target type keeps its qualifiers. A
// runtime-behavior diagnostic: emitted at the end of a function body, not
// in unreachable code, not in a constant-expression or the initializer of
// a namespace-scope constexpr variable. Not for an explicit cast, a value
// that fits, a conversion to bool, or a compound assignment.
//@ strict-diags
//@ diag: 38:18 warning "implicit conversion from 'int' to 'signed char' changes value from 300 to 44"
//@ diag: 39:12 warning "implicit conversion from 'int' to 'short' changes value from 100000 to -31072"
//@ diag: 42:18 warning "implicit conversion from 'int' to 'signed char' changes value from 200 to -56"
//@ diag: 46:18 warning "implicit conversion from 'int' to 'signed char' changes value from -129 to 127"
//@ diag: 48:11 warning "implicit conversion from 'long long' to 'int' changes value from 4294967296 to 0"
//@ diag: 50:22 warning "implicit conversion from 'int' to 'unsigned short' changes value from 65536 to 0"
//@ diag: 51:11 warning "implicit conversion from 'long' to 'int' changes value from 2147483648 to -2147483648"
//@ diag: 54:25 warning "implicit conversion from 'int' to 'const signed char' changes value from 300 to 44"
//@ diag: 55:22 warning "implicit conversion from 'int' to 'volatile short' changes value from 100000 to -31072"
//@ diag: 58:19 warning "implicit conversion from 'E' to 'signed char' changes value from 1000 to -24"
//@ diag: 59:26 warning "implicit conversion from 'int' to 'signed char' changes value from 300 to 44"
//@ diag: 60:23 warning "implicit conversion from 'int' to 'signed char' changes value from 297 to 41"
//@ diag: 62:19 warning "implicit conversion from 'int' to 'signed char' changes value from 200 to -56"
//@ diag: 63:15 warning "implicit conversion from 'int' to 'char8_t' changes value from 300 to 44"
//@ diag: 71:19 warning "implicit conversion from 'int' to 'signed char' changes value from 300 to 44"
//@ diag: 72:7 warning "implicit conversion from 'int' to 'signed char' changes value from 400 to -112"
//@ diag: 73:5 warning "implicit conversion from 'int' to 'signed char' changes value from 500 to -12"
//@ diag: 75:11 warning "implicit conversion from 'int' to 'signed char' changes value from 300 to 44"
//@ diag: 76:29 warning "implicit conversion from 'int' to 'const signed char' changes value from 300 to 44"
//@ diag: 78:17 warning "implicit conversion from 'int' to 'signed char' changes value from 600 to 88"
//@ diag: 83:37 warning "implicit conversion from 'int' to 'signed char' changes value from 300 to 44"
//@ diag: 84:15 note "in instantiation of function template specialization 't<signed char>' requested here"
//@ diag: 87:43 warning "implicit conversion from 'int' to 'signed char' changes value from 300 to 44"
//@ diag: 88:22 note "in instantiation of function template specialization 'n<300>' requested here"
//@ diag: 91:28 warning "implicit conversion from 'int' to 'signed char' changes value from 300 to 44"
signed char g1 = 300;
short g2 = 100000;
unsigned char g3 = -1;
bool g4 = 2;
signed char g5 = 200;
signed char g6 = 0xC8;
signed char g7 = 0b11001000;
signed char g8 = 0200;
signed char g9 = -129;
signed char g10 = 127;
int g11 = 4294967296LL;
unsigned g12 = 4294967295LL;
unsigned short g13 = 65536;
int g14 = 2147483648;
int g15 = 0x80000000LL;
int g16 = 0xFFFFFFFF;
const signed char g17 = 300;
volatile short g18 = 100000;
constexpr signed char g19 = 300;
enum E { A = 1000 };
signed char g20 = A;
signed char g21 = 1000 ? 300 : 1;
signed char g22 = 'a' + 200;
signed char g23 = (signed char)300;
signed char g24 = -(-200);
char8_t g25 = 300;

constexpr signed char id(signed char c) { return c; }
static_assert(id(300) == 44);
int arr[id(300)];

void f(signed char);
signed char h(int x, signed char &r) {
  signed char a = 300;
  a = 400;
  f(500);
  r += 300;
  r = x ? 300 : 1;
  constexpr signed char c = 300;
  (void)a; (void)c;
  if (x) return 600;
  return 0;
  r = 700;
}

template <class T> void t() { T v = 300; (void)v; }
template void t<signed char>();
template void t<int>();

template <int N> signed char n() { return N; }
template signed char n<300>();
template signed char n<3>();

struct S { signed char m = 300; };
