// [class.bit]/1, [conv.integral]/3 (WP-234): an integral constant stored
// into a bit-field too narrow for its value is reported, with the value the
// bit-field takes (-Wbitfield-constant-conversion, on by default; a 1 stored
// into a one-bit signed bit-field under -Wsingle-bit-bitfield-constant-
// conversion): by a simple assignment (not a compound one), at the operator,
// and by the initialization of the member: aggregate and parenthesized
// aggregate initialization, at the initializer; a mem-initializer, at the
// member name; a default member initializer, at the initializer. A braced
// scalar initializer is reported at its element and again at its brace. A
// negated or complemented constant needs only its significant bits; a bool
// bit-field takes any value. Templates are checked at the definition and in
// each instantiation.
//@ args: -Wno-braced-scalar-init
//@ strict-diags
//@ diag: 95:15 warning "implicit truncation from 'int' to bit-field changes value from 8 to 0"
//@ diag: 96:20 warning "implicit truncation from 'int' to bit-field changes value from 6 to 2"
//@ diag: 96:18 warning "implicit truncation from 'unsigned int' to bit-field changes value from 6 to 2"
//@ diag: 102:9 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 102:15 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 102:23 warning "implicit truncation from 'E' to bit-field changes value from 9 to 1"
//@ diag: 102:30 warning "implicit truncation from 'SE' to bit-field changes value from 7 to 3"
//@ diag: 103:14 warning "implicit truncation from 'int' to bit-field changes value from 10 to 2"
//@ diag: 103:12 warning "implicit truncation from 'int' to bit-field changes value from 10 to 2"
//@ diag: 104:13 warning "implicit truncation from 'unsigned int' to bit-field changes value from 4294967295 to 15"
//@ diag: 112:46 warning "implicit truncation from 'int' to bit-field changes value from 100 to 4"
//@ diag: 116:7 warning "implicit truncation from 'int' to bit-field changes value from 8 to 0"
//@ diag: 119:7 warning "implicit truncation from 'int' to bit-field changes value from -5 to 3"
//@ diag: 120:7 warning "implicit truncation from 'int' to bit-field changes value from -5 to 3"
//@ diag: 121:7 warning "implicit truncation from 'int' to bit-field changes value from 4 to -4"
//@ diag: 122:7 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 124:7 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 127:7 warning "implicit truncation from 'unsigned int' to bit-field changes value from 4294967280 to 0"
//@ diag: 130:7 warning "implicit truncation from 'int' to a one-bit wide bit-field changes value from 1 to -1"
//@ diag: 132:7 warning "implicit truncation from 'E' to bit-field changes value from 9 to 1"
//@ diag: 133:8 warning "implicit truncation from 'SE' to bit-field changes value from 7 to 3"
//@ diag: 134:7 warning "implicit truncation from 'unsigned char' to bit-field changes value from 200 to 0"
//@ diag: 135:7 warning "implicit truncation from 'char' to bit-field changes value from 120 to 0"
//@ diag: 136:7 warning "implicit truncation from 'int' to bit-field changes value from 300 to 12"
//@ diag: 137:9 warning "implicit truncation from 'unsigned long long' to bit-field changes value from 2199023255552 to 0"
//@ diag: 139:7 warning "implicit truncation from 'const int' to bit-field changes value from 20 to 0"
//@ diag: 140:13 warning "implicit truncation from 'int' to bit-field changes value from 7 to -1"
//@ diag: 141:7 warning "implicit truncation from 'int' to bit-field changes value from 2 to -2"
//@ diag: 143:7 warning "implicit truncation from 'int' to bit-field changes value from -3 to 1"
//@ diag: 145:7 warning "implicit truncation from 'int' to bit-field changes value from -3 to 1"
//@ diag: 146:9 warning "implicit truncation from 'int' to bit-field changes value from 6 to -2"
//@ diag: 147:16 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 149:5 warning "implicit truncation from 'int' to bit-field changes value from 6 to -2"
//@ diag: 150:7 warning "implicit truncation from 'int' to bit-field changes value from 4 to 0"
//@ diag: 151:13 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 152:9 warning "implicit truncation from 'int' to bit-field changes value from 6 to -2"
//@ diag: 158:7 warning "implicit truncation from 'int' to bit-field changes value from 4 to 0"
//@ diag: 158:10 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 158:24 warning "implicit truncation from 'int' to bit-field changes value from 7 to -1"
//@ diag: 158:45 warning "implicit truncation from 'int' to bit-field changes value from 4 to 0"
//@ diag: 158:53 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 159:16 warning "implicit truncation from 'int' to bit-field changes value from 8 to 0"
//@ diag: 160:11 warning "implicit truncation from 'int' to bit-field changes value from 4 to 0"
//@ diag: 160:10 warning "implicit truncation from 'int' to bit-field changes value from 4 to 0"
//@ diag: 161:9 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 161:19 warning "implicit truncation from 'unsigned long long' to bit-field changes value from 2199023255552 to 0"
//@ diag: 162:7 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 163:8 warning "implicit truncation from 'C8' to bit-field changes value from 100 to 4"
//@ diag: 163:15 warning "implicit truncation from 'int' to bit-field changes value from -9 to 7"
//@ diag: 164:8 warning "implicit truncation from 'int' to bit-field changes value from 8 to 0"
//@ diag: 164:12 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 164:11 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 165:11 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 166:8 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 167:7 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 167:11 warning "implicit truncation from 'int' to bit-field changes value from 6 to -2"
//@ diag: 167:14 warning "implicit truncation from 'int' to bit-field changes value from 7 to -1"
//@ diag: 168:7 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 168:11 warning "implicit truncation from 'int' to bit-field changes value from 6 to -2"
//@ diag: 168:14 warning "implicit truncation from 'int' to bit-field changes value from 7 to -1"
//@ diag: 169:12 warning "implicit truncation from 'int' to bit-field changes value from 6 to -2"
//@ diag: 169:15 warning "implicit truncation from 'int' to bit-field changes value from 7 to -1"
//@ diag: 169:7 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 170:18 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 170:21 warning "implicit truncation from 'int' to bit-field changes value from 4 to 0"
//@ diag: 171:14 warning "implicit truncation from 'int' to bit-field changes value from 4 to 0"
//@ diag: 171:17 warning "implicit truncation from 'int' to bit-field changes value from 4 to 0"
//@ diag: 171:22 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 111:46 warning "implicit truncation from 'int' to bit-field changes value from 5 to 1"
//@ diag: 172:8 note "in instantiation of member function 'T<2>::T' requested here"
//@ diag: 112:37 warning "implicit truncation from 'int' to bit-field changes value from 9 to 1"
//@ diag: 174:3 note "in instantiation of function template specialization 't<9>' requested here"
//@ diag: 112:46 warning "implicit truncation from 'int' to bit-field changes value from 100 to 4"
//@ diag: 112:37 warning "implicit truncation from 'int' to bit-field changes value from 10 to 2"
//@ diag: 175:3 note "in instantiation of function template specialization 't<10>' requested here"
//@ diag: 112:46 warning "implicit truncation from 'int' to bit-field changes value from 100 to 4"
enum E { E0, E9 = 9 };
enum class SE : unsigned { A = 7 };
enum class C8 : signed char { X = 100 };
struct S {
  int a : 3 = 8;
  unsigned b : 2 { 6 };
  E e : 2;
  SE se : 2;
  int d : 1;
  unsigned u : 4;
  bool c : 1;
  S() : a(9), b((5)), e(E9), se(SE::A), d(true), u(~0u), c(true) {}
  S(int) : a{10} {}
  S(char) : u{~0u} {}
};
struct P { int a : 2; int b : 2; };
struct B : P { unsigned char c : 4; unsigned long long big : 40; };
union U { int x : 3; unsigned y : 2; };
struct H { int a : 2; P p; };
struct Q { C8 c : 4; int z : 4; };
template <int N> struct T { int a : N; T() : a(5) {} };
template <int N> void t(S &s) { s.a = N; s.u = 100; }
constexpr int K = 20;

void assign(S &s, B &b, U &u, P &p) {
  s.a = 8;
  s.a = 3;
  s.a = -4;
  s.a = -5;
  s.a = -(5);
  s.a = 2 + 2;
  s.a = true ? 9 : 1;
  s.a += 100;
  s.b = 5;
  s.b = -1;
  s.u = ~0u;
  s.u = ~15u;
  s.u = -1;
  s.c = 2;
  s.d = 1;
  s.d = -1;
  s.e = E9;
  s.se = SE::A;
  s.a = (unsigned char)200;
  s.a = 'x';
  b.c = 300;
  b.big = 1ull << 41;
  b.big = 0xFFFFFFFFFFull;
  b.a = K;
  b.a = b.b = 7;
  b.a = 2;
  b.a = -2;
  b.a = -3;
  b.a = ~1;
  b.a = ~2;
  (b.a) = (6);
  int i = (b.a = 5);
  auto &[x, y] = p;
  x = 6;
  u.y = 4;
  u = {.y = 5};
  p = P{6, 1};
  (void)i;
}

void init() {
  S s;
  P q{4, 5}, r = {.a = 7, .b = 1}, w[2] = {{4}, {1, 9}};
  P *n = new P{8, 0};
  P nest{{4}, 1};
  B bb{{9, 1}, 3, 1ull << 41};
  U v{9};
  Q qq{C8::X, -9};
  P pp(8, {9});
  P pr(1, (5));
  U pu(9);
  H h{5, {6, 7}};
  H g(5, {6, 7});
  H k(5, P(6, 7));
  P arr[2] = {1, 9, 4};
  P arr2[2]({4, 4}, {5});
  T<2> t2;
  T<8> t8;
  t<9>(s);
  t<10>(s);
  (void)n;
}

// Plain `char` is signed on x86_64 and unsigned on AArch64, so the char-typed
// cases above use `unsigned char` to keep the reported values target-neutral.
