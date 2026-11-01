// [conv.fpint]/1: a floating literal implicitly converted to an integer
// type truncates; clang's -Wliteral-conversion reports a changed value
// (and an out-of-range one, [conv.fpint]/1 "undefined"), but not an
// exactly representable one, nor an explicit cast or a non-literal.
//@ strict-diags
//@ diag: 40:9 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 43:14 warning "implicit conversion from 'double' to 'int' changes value from 2.5 to 2"
//@ diag: 44:9 warning "implicit conversion from 'double' to 'int' changes value from -1.5 to -1"
//@ diag: 45:10 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 46:11 warning "implicit conversion of out of range value from 'double' to 'short' is undefined"
//@ diag: 47:9 warning "implicit conversion from 'float' to 'int' changes value from 1.5 to 1"
//@ diag: 48:10 warning "implicit conversion from 'double' to 'bool' changes value from 0.5 to true"
//@ diag: 49:10 warning "implicit conversion from 'double' to 'char' changes value from 65.25 to 65"
//@ diag: 50:14 warning "implicit conversion from 'double' to 'unsigned int' changes value from 1.75 to 1"
//@ diag: 51:10 warning "implicit conversion of out of range value from 'double' to 'long' is undefined"
//@ diag: 57:9 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 58:9 warning "implicit conversion from 'double' to 'int' changes value from 123456789.5 to 123456789"
//@ diag: 59:11 warning "implicit conversion of out of range value from 'double' to 'int' is undefined"
//@ diag: 60:10 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 62:15 warning "implicit conversion from 'double' to 'unsigned int' changes value from -0.5 to 0"
//@ diag: 63:15 warning "implicit conversion of out of range value from 'double' to 'unsigned int' is undefined"
//@ diag: 64:11 warning "implicit conversion from 'double' to 'bool' changes value from 2 to true"
//@ diag: 65:11 warning "implicit conversion from 'double' to 'bool' changes value from -0 to false"
//@ diag: 66:10 warning "implicit conversion from 'double' to 'int' changes value from 1.5E-4 to 0"
//@ diag: 67:16 warning "implicit conversion from 'double' to 'long long' changes value from 1500000000000001 to 1500000000000000"
//@ diag: 68:10 warning "implicit conversion from 'double' to 'int' changes value from 1.0E-5 to 0"
//@ diag: 69:10 warning "implicit conversion from 'double' to 'int' changes value from 2.5 to 2"
//@ diag: 70:10 warning "implicit conversion from 'double' to 'int' changes value from -2.5 to -2"
//@ diag: 72:16 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1"
//@ diag: 73:20 warning "implicit conversion from 'double' to 'int' changes value from 4.5 to 4"
//@ diag: 76:7 warning "implicit conversion from 'double' to 'int' changes value from 5.5 to 5"
//@ diag: 77:14 warning "implicit conversion from 'double' to 'bool' changes value from 0.5 to true"
//@ diag: 80:12 warning "implicit conversion from 'double' to 'myint' (aka 'int') changes value from 6.5 to 6"
//@ diag: 81:35 warning "implicit conversion from 'double' to 'int' changes value from 7.5 to 7"
//@ diag: 82:10 note "in instantiation of function template specialization 'tf<int>' requested here"
struct G { int a, b; };
struct H { G g; };
void f(int);
int f2(int);
int i = 1.5;
int j = 2.0;
H h{{1, 2}};
void t() { f(2.5); f(3.0); }
int k = -1.5;
int m = (1.5);
short s = 1e10;
int n = 1.5f;
bool b = 0.5;
char c = 65.25;
unsigned u = 1.75;
long l = 1e100;
int p = 1.5 + 1;
int q = static_cast<int>(1.5);
int r = int(1.5);
double d = 1.5;
int w = d;
int y = 0x1.8p0;
int z = 123456789.5;
int big = 1e20;
int e2 = 1.5e0;
unsigned long sz = sizeof(f2(1.5));
unsigned uu = -0.5;
unsigned uv = -1.5;
bool b2 = 2.0;
bool b3 = -0.0;
int sm = 0.00015;
long long l2 = 1500000000000000.5;
int nn = 1e-5;
int pl = +2.5;
int ng = -(2.5);
bool cnd;
int co = cnd ? 1.5 : 2;
int ret() { return 4.5; }
void as() {
  int a;
  a = 5.5;
  bool bb = !0.5;
}
typedef int myint;
myint mi = 6.5;
template<class T> T tf() { return 7.5; }
int tv = tf<int>();
