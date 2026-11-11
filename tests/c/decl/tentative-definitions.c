// C17 6.9.2p2: a file-scope object declaration without an initializer
// and without a storage-class specifier (or with static) is a tentative
// definition; repeated ones are fine, and at the end of the translation
// unit it becomes a definition initialized to zero. A later definition
// with an initializer takes precedence.
//@ expect: run
//@ stdout: 0 5 0 0
int printf(const char *, ...);
int a;
int a;
int b;
int b = 5;
int b;
static int c;
static int c;
int arr[3];
int arr[3];
struct P { int x, y; } pt;
int main(void) {
  printf("%d %d %d %d\n", a, b, c, arr[0] + pt.x + pt.y);
  return 0;
}
