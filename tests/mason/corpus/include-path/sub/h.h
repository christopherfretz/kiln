// __FILE__ through an assert-like macro
void fail(const char *, int);
#define CHECK(x) ((x) ? (void)0 : fail(__FILE__, __LINE__))
inline int f(int a) {
  CHECK(a > 0);
  return a;
}
