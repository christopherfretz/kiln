// [basic.scope.block] + [basic.lookup]: a name declared in a block hides
// the same name from an enclosing scope until the end of the block.
//@ expect: run
//@ stdout: 1 2 3 2 1 10
extern "C" int printf(const char*, ...);

int x = 10;

int main() {
  int outer = x;
  int x = 1;
  printf("%d ", x);
  {
    int x = 2;
    printf("%d ", x);
    for (int x = 3; x < 4; ++x) printf("%d ", x);
    printf("%d ", x);
  }
  printf("%d %d\n", x, outer);
}
