// GNU local labels (an extension; [stmt.label], [stmt.goto]): a label
// declared by `__label__` at the start of a block is a distinct label of
// the function, found by `goto`, `&&` and its definition only inside the
// block, so a macro whose statement expression defines one can be
// expanded twice in a function; an inner block's local label shadows an
// outer one and the function's label of the same name. A function
// template's local labels are redeclared in each instantiation.
// gcc.c-torture: compile/20000326-2.c, execute/930406-1.c,
// execute/980526-1.c.
//@ expect: run
//@ stdout: 3 10 | 2 7 | 11 | 2 6 | 4 9
#include <stdio.h>

#define FIRST_POS(arr, n)                                                     \
  ({                                                                          \
    __label__ found;                                                          \
    int i_;                                                                   \
    for (i_ = 0; i_ < (n); i_++)                                              \
      if ((arr)[i_] > 0)                                                      \
        goto found;                                                           \
    i_ = -1;                                                                  \
  found:                                                                      \
    i_;                                                                       \
  })

static int twice() {
  int a[] = {0, -1, 0, 4, 5};
  int b[] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1};
  return FIRST_POS(a, 5) * 100 + FIRST_POS(b, 11);
}

// An inner local label shadows the outer one and the function's label.
static int shadow(int x) {
  int r = 0;
  {
    __label__ out;
    {
      __label__ out;
      if (x)
        goto out;
      r += 100;
    out:
      r += 1;
    }
    goto out;
    r += 1000;
  out:
    r += 1;
  }
  goto out;
  r += 10000;
out:
  return r;
}

// `&&label` and `goto *` on local labels.
static int computed(int k) {
  int r = 0;
  {
    __label__ l0, l1, done;
    static void *tab[] = {&&l0, &&l1};
    goto *tab[k];
  l0:
    r = 2;
    goto done;
  l1:
    r = 7;
  done:;
  }
  return r;
}

template <class T> T tsum(T n) {
  T s = 0;
  {
    __label__ again;
    T i = 0;
  again:
    s += i;
    if (++i <= n)
      goto again;
  }
  {
    __label__ again;
    if (s > 100)
      goto again;
    s += 1;
  again:;
  }
  return s;
}

// Two blocks with the same local label in one function.
static int pair(int x) {
  int r = 0;
  {
    __label__ skip;
    if (x & 1)
      goto skip;
    r += 1;
  skip:;
  }
  {
    __label__ skip;
    if (x & 2)
      goto skip;
    r += 3;
  skip:;
  }
  return r;
}

int main() {
  int t = twice();
  printf("%d %d | ", t / 100, t % 100);
  printf("%d %d | ", computed(0), computed(1));
  printf("%d | ", tsum(4));
  printf("%d %d | ", shadow(1), pair(2) + pair(0) + 1);
  printf("%d %d\n", pair(0), (int)tsum<long>(3) + 2);
  return 0;
}
