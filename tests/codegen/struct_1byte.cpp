// By-value class of 1 byte: x86-64 INTEGER class as i8; AArch64 i64.
struct S1 { char c; };
S1 ext(char);
char take(S1 s) { return s.c; }
S1 pass(char c) { return ext(c); }
int use(char c) {
  S1 local = ext(c);
  return take(ext(c)) + take(pass(c)) + local.c;
}
