//@ expect: ast
enum Op { Add, Sub, Neg };
int eval(Op op, int a, int b) {
  switch (int t = a; op) {
  case Add:
    return t + b;
  case Sub:
  case Neg:
    a = -a;
    a += 1;
  default:
    break;
  }
  switch (b) case 1: return 1;
  return a;
}
