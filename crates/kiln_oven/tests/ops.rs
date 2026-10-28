//! Positive coverage: every M2 operation evaluates to clang's value.
//! Each test names the section that specifies the operation.

mod common;
use common::*;

/// [expr.add], [expr.mul]: integer arithmetic of every width.
#[test]
fn expr_mul_add_widths() {
    ok(r"
static_assert(7 * 6 == 42);
static_assert(7 / 2 == 3 && -7 / 2 == -3 && 7 % 3 == 1 && -7 % 3 == -1);
static_assert(2147483647 - 1 == 2147483646);
static_assert(4294967295u + 1u == 0u);
static_assert(9223372036854775807L - 1 == 9223372036854775806L);
static_assert(18446744073709551615ul + 1 == 0);
static_assert((unsigned char)255 + 1 == 256);
static_assert((short)-32768 - 1 == -32769);
static_assert((__int128)1 << 100 > 0);
static_assert(((unsigned __int128)1 << 127) * 2 == 0);
static_assert((__int128)9223372036854775807L * 4 / 4 == 9223372036854775807L);
static_assert(-(unsigned)1 == 4294967295u);
static_assert(1u - 2 == 4294967295u);
static_assert(-1 < 0u == false);
static_assert(-1L < 0u);
");
}

/// [expr.shift]: shifts, including of the promoted left operand.
#[test]
fn expr_shift() {
    ok(r"
static_assert((1 << 31) == -2147483647 - 1);
static_assert(-8 >> 1 == -4);
static_assert(0xF0u >> 4 == 0xF);
static_assert((unsigned char)1 << 8 == 256);
static_assert(1LL << 63 < 0);
static_assert(((unsigned __int128)1 << 127) >> 127 == 1);
static_assert(1 << 2L == 4);
");
}

/// [expr.bit.and], [expr.or], [expr.xor], [expr.unary.op]: bitwise and
/// unary operators.
#[test]
fn expr_bitwise_unary() {
    ok(r"
static_assert((0xF0 & 0x3C) == 0x30 && (0xF0 | 0x0F) == 0xFF && (0xFF ^ 0x0F) == 0xF0);
static_assert(~0 == -1 && ~0u == 4294967295u);
static_assert(-(-5) == 5 && +3 == 3);
static_assert(!0 && !!7 && !false);
static_assert(~(unsigned char)0 == -1);
");
}

/// [expr.rel], [expr.eq], [expr.log.and], [expr.log.or]: comparisons and
/// short-circuiting.
#[test]
fn expr_rel_logical() {
    ok(r"
constexpr int boom() { return 1 / 0; }
static_assert(1 < 2 && 2 <= 2 && 3 > 2 && 3 >= 3 && 1 != 2 && 2 == 2);
static_assert(!(false && boom()));
static_assert(true || boom());
static_assert((true ? 1 : boom()) == 1);
static_assert('a' < 'b');
static_assert(1.5 < 2 && 2.0f == 2);
");
}

/// [expr.cond]: the conditional operator, including glvalue results.
#[test]
fn expr_cond() {
    ok(r"
constexpr int pick(bool b) { int x = 1, y = 2; (b ? x : y) = 9; return x * 10 + y; }
static_assert(pick(true) == 92 && pick(false) == 19);
static_assert((1 ? 2 : 3.0) == 2.0);
static_assert(sizeof(1 ? (char)1 : (char)2) == 1);
");
}

/// [expr.comma]: the comma operator.
#[test]
fn expr_comma() {
    ok(r"
constexpr int f() { int a = 0; int b = (a = 3, a + 1); return a * 10 + b; }
static_assert(f() == 34);
");
}

/// [expr.ass]: simple and compound assignment of every operator.
#[test]
fn expr_assign() {
    ok(r"
constexpr int f() {
  int x = 10;
  x += 5; x -= 3; x *= 4; x /= 6; x %= 5;
  int y = 0xF0; y &= 0x3C; y |= 1; y ^= 0x10; y <<= 2; y >>= 1;
  return x * 1000 + y;
}
static_assert(f() == 3066);
constexpr unsigned char g() { unsigned char c = 250; c += 10; return c; }
static_assert(g() == 4);
constexpr int h() { int i = 7; i *= 2.5; return i; }
static_assert(h() == 17);
constexpr int k() { int a, b; a = b = 4; return a + b; }
static_assert(k() == 8);
");
}

/// [expr.pre.incr], [expr.post.incr]: increment and decrement.
#[test]
fn expr_incr() {
    ok(r"
constexpr int f() { int i = 5; int a = i++; int b = ++i; int c = i--; int d = --i; return a*1000 + b*100 + c*10 + d; }
static_assert(f() == 5775);
constexpr bool g() { unsigned char c = 255; c++; return c == 0; }
static_assert(g());
constexpr double h() { double d = 1.5; d++; return d; }
static_assert(h() == 2.5);
");
}

/// [expr.cast], [expr.static.cast], [conv.integral]: integral conversions
/// wrap modulo 2^N.
#[test]
fn conv_integral() {
    ok(r"
static_assert((unsigned char)300 == 44);
static_assert((signed char)200 == -56);
static_assert((short)65535 == -1);
static_assert((int)4294967296L == 0);
static_assert((unsigned)-1 == 4294967295u);
static_assert(static_cast<long>(-1) == -1L);
static_assert((bool)256 == true && (bool)0 == false);
static_assert((unsigned __int128)-1 == ~(unsigned __int128)0);
static_assert((int)(__int128)-5 == -5);
");
}

/// [conv.fpint], [conv.double]: floating conversions.
#[test]
fn conv_floating() {
    ok(r"
static_assert((int)3.99 == 3 && (int)-3.99 == -3);
static_assert((unsigned)3.5f == 3);
static_assert((double)1 / 4 == 0.25);
static_assert((float)0.1 != 0.1);
static_assert((double)(float)0.5 == 0.5);
static_assert((long double)1 / 3 != (double)1 / 3);
static_assert((bool)0.0 == false && (bool)-0.5);
static_assert((long)1e18 == 1000000000000000000L);
");
}

/// [basic.fundamental], [expr.sizeof], [expr.alignof]: sizes on x86-64.
#[test]
fn expr_sizeof_alignof() {
    ok(r"
struct S { char c; int i; double d; };
union U { char c[5]; int i; };
static_assert(sizeof(char) == 1 && sizeof(short) == 2 && sizeof(int) == 4);
static_assert(sizeof(long) == 8 && sizeof(long long) == 8 && sizeof(__int128) == 16);
static_assert(sizeof(float) == 4 && sizeof(double) == 8 && sizeof(long double) == 16);
static_assert(sizeof(int*) == 8 && sizeof(struct S) == 16 && alignof(struct S) == 8);
static_assert(sizeof(union U) == 8 && alignof(long double) == 16);
static_assert(sizeof(int[10]) == 40);
constexpr int a[7] = {};
static_assert(sizeof a == 28 && sizeof(a) / sizeof(a[0]) == 7);
static_assert(sizeof(bool) == 1 && sizeof(wchar_t) == 4 && sizeof(char16_t) == 2);
");
}

/// [lex.ccon], [lex.string]: character and string literals of every
/// encoding.
#[test]
fn lex_literals() {
    ok(r#"
static_assert('a' == 97 && '\n' == 10 && '\x41' == 65 && '\101' == 65 && '\0' == 0);
static_assert(u'x' == 120 && U'\x1F600' == 0x1F600 && L'z' == 122 && u8'q' == 113);
static_assert(sizeof('a') == 1);
static_assert(sizeof("abc") == 4);
static_assert("abc"[1] == 'b' && "abc"[3] == 0);
static_assert(sizeof(u"ab") == 6 && U"ab"[1] == U'b');
static_assert(*("xy" + 1) == 'y');
static_assert("ab" "cd"[3] == 'd');
static_assert((char)-1 == -1);
"#);
}

/// [stmt.if], [stmt.while], [stmt.do], [stmt.for]: control flow.
#[test]
fn stmt_loops() {
    ok(r"
constexpr int sum(int n) { int s = 0; for (int i = 1; i <= n; ++i) s += i; return s; }
static_assert(sum(100) == 5050);
constexpr int w(int n) { int c = 0; while (n > 1) { n = n % 2 ? 3 * n + 1 : n / 2; ++c; } return c; }
static_assert(w(27) == 111);
constexpr int d() { int i = 0; do { ++i; } while (i < 0); return i; }
static_assert(d() == 1);
constexpr int bc() {
  int s = 0;
  for (int i = 0; ; ++i) { if (i == 10) break; if (i % 2) continue; s += i; }
  return s;
}
static_assert(bc() == 20);
constexpr int nested() {
  int s = 0;
  for (int i = 0; i < 4; ++i) for (int j = 0; j < 4; ++j) { if (j > i) break; s += j; }
  return s;
}
static_assert(nested() == 10);
constexpr int ifs(int x) { if (x < 0) return -1; else if (x == 0) return 0; else return 1; }
static_assert(ifs(-5) == -1 && ifs(0) == 0 && ifs(9) == 1);
");
}

/// [stmt.switch]: switch with fallthrough, default and nesting.
#[test]
fn stmt_switch() {
    ok(r"
constexpr int f(int x) {
  int r = 0;
  switch (x) {
  case 1: r += 1;
  case 2: r += 10; break;
  case 3: { r = 300; break; }
  default: r = -1;
  case 4: r += 4;
  }
  return r;
}
static_assert(f(1) == 11 && f(2) == 10 && f(3) == 300 && f(4) == 4 && f(9) == 3);
constexpr int g(char c) { switch (c) { case 'a': return 1; case 'b': return 2; } return 0; }
static_assert(g('a') == 1 && g('b') == 2 && g('z') == 0);
constexpr int h(long v) {
  switch (v) { case 5000000000L: return 1; case -1: switch (v + 1) { case 0: return 7; } }
  return 0;
}
static_assert(h(5000000000L) == 1 && h(-1) == 7 && h(3) == 0);
constexpr int k(int n) { int s = 0; for (int i = 0; i < n; ++i) switch (i % 3) { case 0: continue; case 1: s += 1; break; default: s += 10; } return s; }
static_assert(k(6) == 22);
");
}

/// [expr.call], [dcl.fct.def]: calls, recursion, mutual recursion and
/// void functions.
#[test]
fn expr_call() {
    ok(r"
constexpr bool is_odd(unsigned n);
constexpr bool is_even(unsigned n) { return n == 0 ? true : is_odd(n - 1); }
constexpr bool is_odd(unsigned n) { return n == 0 ? false : is_even(n - 1); }
static_assert(is_even(100) && is_odd(51));
constexpr void set(int *p, int v) { *p = v; }
constexpr int use() { int x = 0; set(&x, 42); return x; }
static_assert(use() == 42);
constexpr int ack(int m, int n) { return m == 0 ? n + 1 : n == 0 ? ack(m - 1, 1) : ack(m - 1, ack(m, n - 1)); }
static_assert(ack(2, 3) == 9);
constexpr long widen(char c, short s, long l) { return c + s + l; }
static_assert(widen(1, 2, 3) == 6);
constexpr int noret() { int x = 1; return x; }
static_assert(noret());
");
}

/// [conv.func], [expr.call]: calls through function pointers.
#[test]
fn expr_call_indirect() {
    ok(r"
constexpr int add(int a, int b) { return a + b; }
constexpr int mul(int a, int b) { return a * b; }
constexpr int apply(int (*f)(int, int), int a, int b) { return f(a, b); }
static_assert(apply(add, 3, 4) == 7 && apply(&mul, 3, 4) == 12);
constexpr int (*table[2])(int, int) = {add, mul};
static_assert(table[1](5, 6) == 30);
static_assert(table[0] == add && table[0] != table[1]);
");
}

/// [expr.unary.op], [expr.sub], [expr.add]: pointers into arrays.
#[test]
fn expr_pointer_arith() {
    ok(r"
constexpr int f() {
  int a[5] = {1, 2, 3, 4, 5};
  int *p = a;
  int *q = &a[4];
  int *end = a + 5;
  int s = 0;
  for (int *i = p; i != end; ++i) s += *i;
  return s * 100 + (q - p) * 10 + (end - q) + *(p + 2) + 2[a] + p[3];
}
static_assert(f() == 1500 + 40 + 1 + 3 + 3 + 4);
constexpr bool cmp() { int a[3] = {}; return &a[0] < &a[2] && a + 3 > a + 1 && &a[1] <= &a[1]; }
static_assert(cmp());
constexpr int g() { int a[2][3] = {{1, 2, 3}, {4, 5, 6}}; int (*r)[3] = a + 1; return (*r)[2] + a[0][1]; }
static_assert(g() == 8);
");
}

/// [expr.ref], [class.mem]: structs, member access, nested aggregates.
#[test]
fn expr_ref_struct() {
    ok(r"
struct P { int x, y; };
struct R { struct P a, b; char tag; };
constexpr int area(struct R r) { return (r.b.x - r.a.x) * (r.b.y - r.a.y); }
constexpr struct R box = {{1, 2}, {4, 6}, 'r'};
static_assert(area(box) == 12 && box.tag == 'r');
constexpr struct R grow(struct R r) { r.b.x += 1; return r; }
static_assert(area(grow(box)) == 16);
constexpr int via_ptr() { struct P p = {3, 4}; struct P *q = &p; q->y = 10; return p.x + p.y; }
static_assert(via_ptr() == 13);
constexpr int copy() { struct P a = {1, 2}; struct P b = {0, 0}; b = a; a.x = 9; return b.x * 10 + b.y; }
static_assert(copy() == 12);
constexpr struct P elided[2] = {1, 2, 3};
static_assert(elided[1].x == 3 && elided[1].y == 0);
constexpr struct P desig = {.y = 7};
static_assert(desig.x == 0 && desig.y == 7);
");
}

/// [class.bit]: bit-fields, signed and unsigned, truncating on store.
#[test]
fn class_bit() {
    ok(r"
struct B { unsigned a : 3; int b : 4; unsigned : 0; unsigned c : 7; bool d : 1; long long e : 40; };
constexpr int f() {
  struct B s = {};
  s.a = 9;
  s.b = 7;
  s.c = 200;
  s.d = true;
  s.e = -1;
  int r = s.a * 1000 + s.c;
  s.b += 1;
  return r + (s.b == -8) + (s.d ? 10000 : 0) + (s.e == -1 ? 100000 : 0);
}
static_assert(f() == 111000 + 72 + 1);
constexpr struct B init = {5, -3, 100, false, 12345678901LL};
static_assert(init.a == 5 && init.b == -3 && init.c == 100 && !init.d && init.e == 12345678901LL);
constexpr unsigned wrap() { struct B s = {}; s.a = 7; s.a++; return s.a; }
static_assert(wrap() == 0);
static_assert(sizeof(struct B) == 16);
");
}

/// [class.union]: unions and the active member.
#[test]
fn class_union() {
    ok(r"
union U { int i; float f; char c[4]; };
constexpr int f() { union U u = {5}; u.f = 1.5f; u.i = 3; return u.i; }
static_assert(f() == 3);
constexpr union U d = {.f = 2.0f};
static_assert(d.f == 2.0f);
struct T { int kind; union U v; };
constexpr struct T t = {1, {.c = {'a', 'b'}}};
static_assert(t.v.c[1] == 'b' && t.v.c[3] == 0);
");
}

/// [dcl.enum]: enumerations, scoped and unscoped, fixed and not.
#[test]
fn dcl_enum() {
    ok(r"
enum Color { Red, Green = 5, Blue };
static_assert(Red == 0 && Green == 5 && Blue == 6);
enum class E : unsigned char { A = 1, B = 255 };
static_assert((int)E::B == 255 && E::A != E::B);
enum Small : short { S1 = -2 };
static_assert(S1 + 1 == -1 && sizeof(enum Small) == 2);
constexpr enum Color next(enum Color c) { return (enum Color)(c + 1); }
static_assert(next(Green) == Blue);
constexpr int sw(enum Color c) { switch (c) { case Red: return 1; case Blue: return 3; default: return 0; } }
static_assert(sw(Blue) == 3);
");
}

/// [dcl.constexpr], [expr.const]/5.9: constexpr variables, including
/// arrays and structs, read in later constant expressions.
#[test]
fn dcl_constexpr_vars() {
    ok(r#"
constexpr int k = 6 * 7;
constexpr int arr[] = {k, k + 1, k + 2};
static_assert(arr[2] == 44 && sizeof arr == 12);
const int ci = k * 2;
static_assert(ci == 84);
constexpr const int *pk = &k;
static_assert(*pk == 42);
constexpr const int *pa = arr + 1;
static_assert(pa[1] == 44 && *(pa - 1) == 42);
constexpr char str[] = "hey";
static_assert(sizeof str == 4 && str[2] == 'y');
constexpr const char *s = "text";
static_assert(s[3] == 't' && s[4] == 0);
constexpr int zero = {};
static_assert(zero == 0);
constexpr int sum() { int s = 0; for (int i = 0; i < 3; ++i) s += arr[i]; return s; }
static_assert(sum() == 129);
"#);
}

/// [stmt.dcl], [basic.stc.static]: static constexpr locals.
#[test]
fn stmt_dcl_static_constexpr_local() {
    ok(r"
constexpr int f(int i) { static constexpr int t[] = {10, 20, 30}; return t[i]; }
static_assert(f(2) == 30);
");
}

/// [expr.prim.literal], [conv.ptr]: nullptr and null pointers.
#[test]
fn conv_ptr_null() {
    ok(r"
constexpr int *np = nullptr;
static_assert(np == nullptr && !np && np == 0);
constexpr const int *z = 0;
static_assert(z == np);
constexpr int k = 1;
static_assert(&k != nullptr && &k);
constexpr bool f(const int *p) { return p ? *p == 1 : false; }
static_assert(f(&k) && !f(nullptr));
");
}

/// [dcl.init.aggr]: local aggregates, value-initialized fillers, arrays of
/// structs, and returning aggregates by value.
#[test]
fn dcl_init_aggr() {
    ok(r"
struct V { int a[3]; char c; };
constexpr struct V mk(int x) { struct V v = {{x, x + 1}}; v.c = 'z'; return v; }
constexpr int f() { struct V v = mk(4); return v.a[0] + v.a[1] + v.a[2] + (v.c == 'z'); }
static_assert(f() == 10);
constexpr int g() { struct V vs[3] = {}; vs[2].a[1] = 7; return vs[2].a[1] + vs[0].a[0]; }
static_assert(g() == 7);
constexpr struct V ret() { return {{1, 2, 3}, 'x'}; }
constexpr struct V rv = ret();
static_assert(rv.c == 'x' && rv.a[2] == 3);
");
}

/// [expr.prim.id], [basic.scope]: shadowing and block scopes.
#[test]
fn basic_scope() {
    ok(r"
constexpr int x = 1;
constexpr int f() { int x = 2; { int x = 3; x += 1; } return x; }
static_assert(f() == 2 && x == 1);
");
}

/// [expr.unary.op], [expr.ref]: pointers to struct members and arrays of
/// pointers.
#[test]
fn pointers_to_members_of_objects() {
    ok(r"
struct N { int v; struct N *next; };
constexpr int walk() {
  struct N c = {3, nullptr};
  struct N b = {2, &c};
  struct N a = {1, &b};
  int s = 0;
  for (struct N *p = &a; p; p = p->next) s = s * 10 + p->v;
  return s;
}
static_assert(walk() == 123);
constexpr int f() { int a = 1, b = 2; int *ps[2] = {&a, &b}; *ps[1] = 5; int **pp = &ps[0]; **pp = 4; return a * 10 + b; }
static_assert(f() == 45);
");
}

/// [basic.fundamental]: long double is x87 80-bit on x86-64 Linux.
#[test]
fn long_double_x87() {
    ok(r"
static_assert(1.0L / 3 * 3 == 1.0L);
static_assert((long double)9007199254740993L == 9007199254740993L);
static_assert((long)(double)9007199254740993L != 9007199254740993L);
");
}

/// [expr.static.cast]: void casts discard.
#[test]
fn expr_cast_void() {
    ok(r"
constexpr int f() { int x = 1; (void)x; static_cast<void>(x + 1); return x; }
static_assert(f() == 1);
");
}

/// [stmt.return]: returning from nested loops and switches.
#[test]
fn stmt_return_nested() {
    ok(r"
constexpr int find(int n) {
  for (int i = 0; i < 10; ++i)
    for (int j = 0; j < 10; ++j)
      switch (i * j) { case 42: if (i + j == n) return i * 10 + j; }
  return -1;
}
static_assert(find(13) == 67 && find(5) == -1);
");
}
