// C17 6.7.2.1p18: the last member of a structure with more than one
// named member may be an array of unknown size (a flexible array
// member); sizeof ignores it (except for padding).
//@ expect: run
//@ stdout: 4 1 2 3
int printf(const char *, ...);
void *malloc(unsigned long);
struct F { int n; int d[]; };
int main(void) {
  struct F *f = malloc(sizeof(struct F) + 3 * sizeof(int));
  f->n = 3;
  for (int i = 0; i < f->n; i++) f->d[i] = i + 1;
  printf("%zu %d %d %d\n", sizeof(struct F), f->d[0], f->d[1], f->d[2]);
  return 0;
}
