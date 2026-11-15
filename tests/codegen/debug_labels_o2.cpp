// Debug info (WP-1445): at -O2 a function's labels are retained nodes
// after its variables (clang `DIBuilder::createLabel(AlwaysPreserve)`).
//@ args: -g -O2 -Xclang -disable-llvm-passes
extern int g;
int k(int a) {
  int b = a * 2;
top:
  g += b;
  if (a-- > 0)
    goto top;
  return a;
}
