//@ expect: ast
extern int defined_later;
extern const int limit;
const int limit = 10;
int defined_later = limit * 2;
int read() {
  extern int defined_later;
  return defined_later;
}
