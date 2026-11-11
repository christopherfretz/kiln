// GNU local labels in C (an extension; C23 6.8.2, 6.8.7.2): `__label__`
// at the start of a block declares labels scoped to that block, so a
// macro whose statement expression jumps to one can be expanded more
// than once per function; `&&label` and `goto *` work on them, and an
// inner declaration shadows an outer one.
// gcc.c-torture: compile/20000326-2.c, compile/20000518-1.c,
// compile/981006-1.c, execute/930406-1.c, execute/980526-1.c.
//@ std: c17
//@ expect: run
//@ stdout: 3 10 | 2 7 | 2 | 4
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

static int twice(void) {
  int a[] = {0, -1, 0, 4, 5};
  int b[] = {0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1};
  return FIRST_POS(a, 5) * 100 + FIRST_POS(b, 11);
}

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

int main(void) {
  int t = twice();
  printf("%d %d | ", t / 100, t % 100);
  printf("%d %d | ", computed(0), computed(1));
  printf("%d | %d\n", shadow(1), pair(0));
  return 0;
}
