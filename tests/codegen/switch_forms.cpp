// switch over int, char, long and enum, with fallthrough and default.
enum K { A = 1, B = 2, C = 4 };
int sw(int x, char c, long l, K k) {
  int r = 0;
  switch (x) { case 0: r = 1; break; case 1: case 2: r = 2; case 3: r += 3; break; default: r = -1; }
  switch (c) { case 'a': r += 10; break; case 'z': r += 26; }
  switch (l) { case 1L << 33: r += 100; break; default: break; }
  switch (k) { case A: case B: r *= 2; break; case C: r *= 4; }
  return r;
}
