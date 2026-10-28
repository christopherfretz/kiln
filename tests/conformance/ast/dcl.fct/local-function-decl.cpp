//@ expect: ast
int helper(int);
int caller() {
  int helper(int);
  extern int shared;
  return helper(shared);
}
int shared = 2;
int helper(int v) { return v + 1; }
