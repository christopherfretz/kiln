//@ expect: ast
//@ ast-filter: counter
// The filter dumps every declaration whose qualified name contains the
// substring: a local static is named through its function's signature.
int counter_next(int step) {
  static int counter = 0;
  counter += step;
  return counter;
}
int other() { return 0; }
