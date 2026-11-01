//@ expect: ast
//@ ast-filter: f
// [stmt.label], [stmt.goto], [dcl.asm]: `LabelStmt` names its label and
// holds the labelled statement; `GotoStmt` names its target. The labels
// of an `asm goto` are not children of the `GCCAsmStmt` (clang's text
// dump prints only the operands), also in a template pattern. Matches
// clang 19.
int f(int x) {
  asm goto("" : : "r"(x) : : out, err);
  if (x)
    goto err;
again:
  x--;
  if (x > 3) goto again;
out:
  return x;
err:
  return -1;
}
template <class T> T ft(T x) {
  asm goto("" : : "r"(x) : : done);
  goto done;
done:
  return x;
}
