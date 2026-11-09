// A GNU case range ([stmt.switch] extension) that follows other case
// labels gets its own block, entered by fallthrough from the statements
// before it, as clang's EmitCaseStmtRange (WP-941: kiln emitted the range
// test into the preceding case's block).
int ext(int);
int classify(int k) {
  switch (k) {
  case 0:
    k = ext(k);
  case 10 ... 20:
    return k + 1;
  case 1:
  case 30 ... 40:
    return 2;
  }
  return 0;
}
