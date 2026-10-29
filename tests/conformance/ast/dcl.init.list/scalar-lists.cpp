// [dcl.init.list] for scalars: braced arguments, returns, assignments.
int take(int);
int ret() { return {7}; }
int ret_empty() { return {}; }
void g(long x) {
  int a = take({1});
  int b{};
  b = {2};
  b += 0;
  long c = {x};
  (void)a; (void)c;
}
