void f(int);
inline int g(int a) {
  if (a)
    f(a);
  return 1;
}
int (*keep_b)(int) = g;
