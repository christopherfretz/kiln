//@ expect: ast
int pick(int v) {
  switch (int w = v * 2; w) {
  case 0: return 0;
  case 2: return 1;
  }
  switch (long x = v) {
  case 1L: return (int)x;
  }
  return -1;
}
