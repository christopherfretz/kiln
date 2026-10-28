//@ expect: ast
extern "C" int c_printf(const char *, ...);
int log(const char *fmt, ...);
void call(float f, short s, char c) {
  c_printf("%f %d %c", f, s, c);
  log("x", 1u, true);
}
