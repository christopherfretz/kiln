// The comma operator and the conditional operator on mixed types.
int g(int);
double ct(int a, double d, int* p, int* q) {
  int x = (g(1), g(2), a);
  double y = a ? d : a;
  int* r = a ? p : q;
  long l = a ? 1 : 2L;
  (a ? *p : *q) = 3;
  return x + y + *r + l;
}
