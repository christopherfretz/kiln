// [expr.new]/22, [dcl.init.string]/3: a braced or parenthesized string
// literal initializes a new array of char from its characters, and the
// remaining elements are zero; also in a constant expression.
constexpr int braced(int n) {
  char *p = new char[n]{"ab"};
  int r = p[0] + p[1] * 2 + p[2] * 3 + p[n - 1];
  delete[] p;
  return r;
}
static_assert(braced(3) == 'a' + 'b' * 2);
static_assert(braced(6) == 'a' + 'b' * 2);

constexpr int parens() {
  char *p = new char[4]("xyz");
  int r = p[2] + p[3];
  delete[] p;
  return r;
}
static_assert(parens() == 'z');

constexpr int wide(int n) {
  char16_t *p = new char16_t[n]{u"été"};
  int r = p[0] + p[2] + p[n - 1];
  delete[] p;
  return r;
}
static_assert(wide(5) == 0xe9 * 2);

constexpr int unbounded() {
  char8_t *p = new char8_t[]{u8"kiln"};
  int r = p[3] + p[4];
  delete[] p;
  return r;
}
static_assert(unbounded() == 'n');
