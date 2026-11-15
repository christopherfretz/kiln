// Debug info (WP-1445): a variable-length array (GNU extension in C++)
// gets a DILocalVariable whose DISubrange counts are the artificial
// `__vla_exprN` variables clang creates for each non-constant bound
// (numbered across the function; a constant bound stays a count).
//@ args: -g -Wno-vla-cxx-extension
int f(int n, int m) {
  int vla[n];
  int v2[n][m];
  int v3[n][4];
  vla[0] = 1;
  v2[0][0] = 2;
  v3[0][0] = 3;
  {
    long inner[m + 1];
    inner[0] = 4;
    vla[0] += (int)inner[0];
  }
  return vla[0] + v2[0][0] + v3[0][0];
}
