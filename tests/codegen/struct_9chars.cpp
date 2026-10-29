// A 9-byte char array member: size 9, alignment 1 ({ i64, i8 } with a
// coercion temporary on x86-64).
struct C9 { char c[9]; };
C9 ext(int);
int take(C9 s) { return s.c[0] + s.c[8]; }
C9 pass(int x) { return ext(x); }
int use(int x) {
  C9 local = ext(x);
  return take(ext(x)) + take(pass(x)) + local.c[4];
}
