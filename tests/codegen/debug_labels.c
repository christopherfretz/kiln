// Debug info (WP-1445): C labels get a DILabel and a `#dbg_label`
// record, as clang emits them.
//@ args: -g -std=gnu17
extern int g;
int k(int a) {
top:
  if (a-- > 0)
    goto top;
  if (a)
    goto done;
  g = 1;
done:
  return a;
}
