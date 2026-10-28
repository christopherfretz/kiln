//@ expect: ast
enum K { A, B, C };
int classify(K k, char c) {
  switch (k) {
  case A:
  case B:
    return 1;
  default:
    break;
  }
  switch (c) {
  case 'a': return 2;
  case 'b' + 1: { int t = 3; return t; }
  }
  return 0;
}
