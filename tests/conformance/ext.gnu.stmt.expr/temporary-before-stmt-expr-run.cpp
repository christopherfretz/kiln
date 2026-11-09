// [class.temporary]/4, [intro.execution]/5 (GNU statement expressions): a
// temporary created before a statement expression in the same
// full-expression is destroyed at the end of that full-expression, not
// with the enclosing block. The statement expression's own statements are
// full-expressions that destroy their own temporaries. Before WP-1081
// kiln let the statement expression's last statement take the enclosing
// full-expression's pending cleanups, so `D{1}` below lived to the end of
// the block ("a1 ~D1"). Also in a template (the instantiation path) and
// with nested statement expressions. spec/sema.md §5.8.
//@ expect: run
//@ stdout: ~D1 a1 ~D2 b2 ~D3 c3 ~D4 d4 | ~D6 ~D5 e5 ~D8 ~D7 f7 | ~D9 t9 ~D9 t9 end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};

template <class T> T in_template(T n) {
  T i = D{int(n)}.v + ({ T(0); });
  printf("t%d ", int(i));
  return i;
}

int main() {
  { int i = D{1}.v + ({ 0; }); printf("a%d ", i); }
  { int i = D{2}.v; printf("b%d ", i); }
  { int i = ({ 0; }) + D{3}.v; printf("c%d ", i); }
  { int i = D{4}.v + ({ int q = 0; q; }); printf("d%d ", i); }
  printf("| ");
  // The inner statement's temporary dies first, at the end of its own
  // full-expression.
  { int i = D{5}.v + ({ int q = D{6}.v; q - 6; }); printf("e%d ", i); }
  { int i = D{7}.v + ({ ({ D{8}.v; }) - 8; }); printf("f%d ", i); }
  printf("| ");
  in_template(9);
  in_template(9L);
  printf("end\n");
}
