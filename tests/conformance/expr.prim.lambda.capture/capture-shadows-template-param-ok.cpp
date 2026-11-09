// [expr.prim.lambda.capture]/5 valid twin: captures whose names differ
// from the lambda's template parameters, a capture-default, and a
// capture named like an enclosing template's parameter all compile
// (WP-1091).
//@ expect: compile-ok
template<class T> int f() {
  int x = 1;
  return [T = 2]() { return T; }() + x;
}
int main() {
  int x = 3;
  auto a = [z = 1]<typename W>(W w) { return z + int(w); };
  auto b = [=]<typename X>(X) { return x; };
  auto c = [x]<int N>() { return x + N; };
  return a(1) + b(0) + c.operator()<1>() + f<int>() - 12;
}
