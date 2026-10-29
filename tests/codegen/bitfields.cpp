// Bit-field reads and writes (no compound assignment, no constants).
struct Flags { unsigned a : 3; unsigned b : 5; int c : 4; unsigned d : 20; };
int bf(Flags* f, int v) {
  f->a = v;
  f->b = 17;
  f->c = -3;
  f->d = f->a + f->b;
  return f->a + f->b + f->c + (int)f->d;
}
