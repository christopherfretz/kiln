// C17 6.7.2.2p3: enumeration constants have type int; p4: an enumerated
// type is compatible with an integer type, and integers convert to and
// from enumerations implicitly (6.3.1.3), including in assignment and
// argument passing.
//@ expect: compile-ok
//@ strict-diags
enum color { RED, GREEN = 5, BLUE };
_Static_assert(_Generic(RED, int: 1, default: 0), "constant is int");
_Static_assert(sizeof(BLUE) == sizeof(int), "");
static enum color next(enum color c) { return c + 1; }
int main(void) {
  enum color c = 1;
  c = 2;
  c++;
  c = next(3);
  int i = c;
  c = i * 2;
  enum color d = GREEN | BLUE;
  return d + i - c == 0;
}
