//@ expect: run
//@ strict-diags
//@ args: -std=gnu++20
//@ stdout: f live=6 sizeof=24 ~5 ~4 ~3 ~2 ~1 ~0 live=0
//@ stdout: f live=0 sizeof=0 live=0
//@ stdout: g live=8 sizeof=32 ~13 ~12 ~11 ~10 ~9 ~8 ~7 ~6 after goto live=0
//@ stdout: f ~3 ~2 ~1 ~0 caught live=0
// [dcl.array] variable length arrays (a GNU extension) whose element type is
// an array of a class type, `S a[n][3]`: the n * 3 elements are
// default-initialized in order ([dcl.init.general]/7.2) and destroyed in
// reverse order when the scope is left ([class.dtor]/14), also by `goto`
// and by an exception from the fifth constructor (only the four built are
// destroyed); an empty array constructs nothing, and `sizeof` is the full
// size. vla-class-elements-run.cpp is the one-dimensional sibling. Found
// running GCC's C++ testsuite.
extern "C" int printf(const char *, ...);

int live, made, throw_at = -1;
struct S {
  int id;
  S() : id(made) {
    if (made == throw_at) throw 1;
    made++;
    live++;
  }
  ~S() {
    live--;
    printf(" ~%d", id);
  }
};

void f(int n) {
  S a[n][3];
  printf("f live=%d sizeof=%d", live, (int)sizeof a);
}

void g(int n, int m) {
  {
    S a[n][2][m];
    printf("g live=%d sizeof=%d", live, (int)sizeof a);
    if (n) goto out;
    printf(" not reached");
  }
out:
  printf(" after goto live=%d\n", live);
}

int main() {
  f(2);
  printf(" live=%d\n", live);
  f(0);
  printf(" live=%d\n", live);
  g(2, 2);
  made = 0;
  throw_at = 4;
  printf("f");
  try {
    f(2);
  } catch (int) {
    printf(" caught live=%d\n", live);
  }
}
