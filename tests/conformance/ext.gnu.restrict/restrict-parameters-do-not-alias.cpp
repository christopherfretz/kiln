// GNU `__restrict__` on pointer parameters promises the pointees do not
// alias, so at -O2 the store through `left` cannot change what `right`
// reads back and the call to the never-defined `unreachable_wp` folds
// away (clang marks such parameters `noalias`). Found running GCC's C++
// testsuite.
//@ expect: run
//@ args: -O2
//@ xfail(kiln): WP-1663: codegen drops `noalias` on __restrict__ pointer parameters, so the unreachable call survives and the link fails
void unreachable_wp();

void fill(int *__restrict__ left, int *__restrict__ right) {
  right[0] = 7;
  left[0] = 1;
  if (right[0] != 7)
    unreachable_wp();
}

int main() {
  int a = 0, b = 0;
  fill(&a, &b);
  return a == 1 && b == 7 ? 0 : 1;
}
