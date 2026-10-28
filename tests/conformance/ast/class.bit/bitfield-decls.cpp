//@ expect: ast
struct Flags {
  unsigned ready : 1;
  unsigned mode : 3;
  int delta : 5;
  unsigned : 0;
  bool b : 1;
};
int read(Flags f) {
  f.mode = 5;
  return f.ready + f.mode + f.delta + f.b;
}
