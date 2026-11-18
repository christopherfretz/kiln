// [expr.cond]/5: a glvalue conditional is a bit-field if either operand
// is one; every access through it uses the selected operand's bit-field
// (or plain access): simple and compound assignment, prefix and postfix
// `++`/`--`, the value of an assignment re-read from the bit-field
// ([expr.ass]/1, truncated and sign-extended), operands in different
// objects, a bit-field and an ordinary `int`, `bool`, enumeration and
// `unsigned` bit-fields, the left operand of a comma, parentheses,
// nested conditionals, a right operand that changes the condition (it is
// sequenced first, [expr.ass]/1), a constant condition, packed and
// zero-width-separated neighbours, a `volatile` object, and a `const int&`
// bound to a temporary copy ([dcl.init.ref]/5.4.2). clang 19 and 22 do
// not generate code for these; compared with GCC 16.1. Found running
// GCC's C++ testsuite (WP-1692).
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19 and 22 do not generate code for a glvalue conditional of bit-fields
//@ args: -w
//@ stdout: -7 904
//@ stdout: s -7 904 5 1 2 17 7
//@ stdout: -6 904 4 17
//@ stdout: s -6 905 4 1 2 16 7
//@ stdout: s -6 100 4 1 2 16 42
//@ stdout: t -3 43 3 0 1 4 9
//@ stdout: 0 3 5 -1
//@ stdout: s -1 100 5 0 2 16 42
//@ stdout: t -3 43 3 0 3 4 9
//@ stdout: s 3 6 5 0 2 16 42
//@ stdout: t -5 1 7 0 3 4 9
//@ stdout: g 11
//@ stdout: s 3 7 5 0 2 16 42
//@ stdout: s 4 11 5 0 2 16 42
//@ stdout: t -5 1 1 0 3 4 9
//@ stdout: x -1 255 y
//@ stdout: 7 11 6 6 8 11
//@ stdout: 4 0
//@ stdout: 3
extern "C" int printf(const char *, ...);

enum E { E0, E1, E2, E3 };
struct S {
  int p : 4;
  int q : 12;
  unsigned r : 3;
  bool b : 1;
  E e : 2;
  int : 0;
  unsigned z : 5;
  int w;
};
struct __attribute__((packed)) P { char c; int a : 7; int b2 : 9; char d; };

S s = {-2, 1000, 5, true, E2, 17, 7};
S t = {1, 2, 3, false, E1, 4, 9};
P pk = {'x', 10, -20, 'y'};
volatile S vs = {3, 4, 5, false, E0, 6, 8};
int g = 11;
int k;

void dump(const char *n, const S &a) {
  printf("%s %d %d %u %d %d %u %d\n", n, a.p, a.q, a.r, (int)a.b, (int)a.e, a.z, a.w);
}
bool side(bool &c) { c = !c; return true; }

int main(int argc, char **) {
  bool c = argc > 0, n = !c;
  // simple assignment, value of the assignment re-read from the bit-field
  int v1 = ((c ? s.p : s.q) = 9);
  int v2 = ((n ? s.p : s.q) = 5000);
  printf("%d %d\n", v1, v2);
  dump("s", s);
  // compound assignment, ++ and -- pre and post
  int v3 = ((c ? s.p : s.q) += 1);
  int v4 = (n ? s.p : s.q)++;
  int v5 = --(c ? s.r : s.z);
  int v6 = (n ? s.r : s.z)--;
  printf("%d %d %d %d\n", v3, v4, v5, v6);
  dump("s", s);
  // operands in different objects; bit-field and an ordinary int
  (c ? s.q : t.p) = 100;
  (n ? s.q : t.p) = -3;
  (c ? s.w : t.q) = 42;
  (n ? s.w : t.q) = 43;
  dump("s", s);
  dump("t", t);
  // bool, enumeration, unsigned truncation, sign extension
  bool bb = ((c ? s.b : t.b) = false);
  E ee = ((n ? s.e : t.e) = E3);
  unsigned uu = ((c ? s.r : t.z) = 77);
  int ss2 = ((c ? s.p : t.q) = 15);
  printf("%d %d %u %d\n", (int)bb, (int)ee, uu, ss2);
  dump("s", s);
  dump("t", t);
  // comma, parentheses, nested conditionals
  ((c ? s.p : s.q), (n ? s.p : s.q)) = 6;
  ((c ? (c ? s.p : s.w) : (n ? s.q : t.p))) = 3;
  (n ? (c ? s.p : s.w) : (n ? s.q : t.p)) = 2;
  (c ? (n ? s.p : t.q) : g) = 1;
  (n ? g : (c ? t.p : s.q)) = -5;
  ((((n ? s.z : t.r)))) = -1;
  dump("s", s);
  dump("t", t);
  printf("g %d\n", g);
  // the right side changes the condition: the right operand is sequenced first
  bool cc = true;
  (cc ? s.p : s.q) += (side(cc), 1);
  dump("s", s);
  // constant-folded condition
  (true ? s.p : s.q) = 4;
  (false ? s.p : s.q) = 11;
  (1 ? t.r : t.z) += 2;
  dump("s", s);
  dump("t", t);
  // packed neighbours
  (c ? pk.a : pk.b2) = -1;
  (n ? pk.a : pk.b2) = 255;
  printf("%c %d %d %c\n", pk.c, pk.a, pk.b2, pk.d);
  // volatile object
  (c ? vs.p : vs.q) = 7;
  (n ? vs.p : vs.q) += 7;
  ++(c ? vs.r : vs.z);
  int rv = (n ? vs.p : vs.q);
  printf("%d %d %u %u %d %d\n", vs.p, vs.q, vs.r, vs.z, vs.w, rv);
  // const reference binds to a temporary
  const int &cr = c ? s.p : s.q;
  s.p = 0;
  printf("%d %d\n", cr, s.p);
  // a discarded read
  (void)(c ? s.p : s.q);
  k = (c ? s.p : s.q) = 3;
  printf("%d\n", k);
}
