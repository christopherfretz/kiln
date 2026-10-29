// [over.match.ctor]: constructor selection for direct- and
// copy-initialization of a class object; explicit constructors only in
// direct-initialization.
//@ expect: compile-ok
struct S {
  int k;
  S(int) : k(1) {}
  S(long) : k(2) {}
  explicit S(double) : k(3) {}
  S(const char *, int = 0) : k(4) {}
};
S a(1);
S b(1L);
S c(1.0);
S d("x");
S e("x", 2);
S f = 1;
S g = "x";
S h = S(2.0);
struct T {
  T(int &);
  T(const int &);
  T(int &&);
};
int i;
const int ci = 0;
T t1(i);
T t2(ci);
T t3(1);
