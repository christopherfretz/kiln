// [expr.new]: scalar new-expressions with no initializer, a parenthesized
// initializer, a braced initializer and value-initialization `()`/`{}`;
// the objects are freed with delete.
//@ expect: run
//@ stdout: 7 9 0 0 2.5 | 42
extern "C" int printf(const char*, ...);

int main() {
  int* a = new int(7);
  int* b = new int{9};
  int* c = new int();
  long* d = new long{};
  double* e = new double(2.5);
  int** pp = new int*(a);
  printf("%d %d %d %ld %.1f | ", *a, *b, *c, *d, *e);
  **pp = 42;
  printf("%d\n", *a);
  delete pp;
  delete e;
  delete d;
  delete c;
  delete b;
  delete a;
}
