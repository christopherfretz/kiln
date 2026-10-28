//@ expect: ast
struct Box { int v; };
int local() {
  int x = 1;
  return x;
}
int param(int p) { return p; }
int global_g;
int global() { return global_g; }
