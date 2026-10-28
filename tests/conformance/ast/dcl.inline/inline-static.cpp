//@ expect: ast
static int counter;
inline int bump() { return ++counter; }
static inline int peek() { return counter; }
int next() {
  static int calls = 0;
  calls++;
  return bump() + peek() + calls;
}
