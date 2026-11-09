//@ expect: run
// [stmt.switch]/2,5, [expr.const] (WP-1144): case and default labels nested
// in loops, blocks and `if` branches give the same results when the
// function is constant-evaluated (each `CHECK`'s template argument) and
// when it runs (the arguments are not constants). The program prints
// every result; the oracle prints the same.
#include <stdio.h>

// Labels in a `for` body: the loop variable lives outside the switch, so
// the jump bypasses no initialization ([stmt.dcl]/3); `++i` and `i < 3` run
// after the first, partial pass.
constexpr int in_for(int x) {
  int r = 0, i = 0;
  switch (x) {
  case 0:
    for (; i < 3; ++i) {
      r += 1;
    case 1:
      r += 10;
    case 2:
      r += 100;
    }
  }
  return r * 10 + i;
}

// Labels in a `while` body, `default` among them.
constexpr int in_while(int x, int n) {
  int r = 0;
  switch (x) {
  default:
    while (n > 0) {
      r = r * 3 + 1;
    case 5:
      r += 2;
      --n;
    }
  }
  return r;
}

// Labels in nested blocks and both branches of an `if`; the jump bypasses
// a block's uninitialized local, which is valid ([stmt.dcl]/3).
constexpr int in_block(int x) {
  int r = 0;
  switch (x) {
  {
    int u;
  case 1:
    u = 11;
    r += u;
    {
    case 2:
      r += 20;
      if (x > 2) {
      case 3:
        r += 300;
      } else {
      case 4:
        r += 4000;
      }
    }
  }
  case 9:
    r += 50000;
  }
  return r;
}

// `continue` and `break` after the jump target their own loop: `continue`
// runs the increment, `break` leaves the loop, not the switch.
constexpr int cont_break(int x) {
  int r = 0, i = 0;
  switch (x) {
  case 0:
    for (; i < 10; ++i) {
      if (i % 2)
        continue;
      r += i;
    case 1:
      r += 100;
      if (i == 3)
        continue;
    case 2:
      if (i >= 6)
        break;
      r += 1000;
    }
    r += 7;
    break;
  case 3:
    r = -1;
  }
  return r * 100 + i;
}

// `continue` in a `do` body entered mid-way goes to the condition.
constexpr int do_continue(int x, int n) {
  int r = 0;
  switch (x) {
  case 0:
    do {
      r += 1;
    case 1:
      if (n % 3 == 0)
        continue;
      r += 10;
    } while (--n > 0);
  }
  return r;
}

// A switch nested in a loop entered by the outer switch; the inner one has
// its own label inside a loop too.
constexpr int nested_switch(int x, int y) {
  int r = 0, k = 0;
  switch (x) {
  case 0:
    do {
      switch (y) {
      case 0:
        r += 1;
        break;
      case 1:
        k = 0;
        for (; k < 2; ++k) {
          r += 2;
        case 2:
          r += 3;
        }
        break;
      default:
        r += 4;
      }
    case 1:
      r += 10;
    } while (++y < 3);
  }
  return r;
}

// The per-iteration scope of a `for` whose increment creates a temporary
// with a destructor.
struct Step {
  int v;
  int *log;
  constexpr ~Step() { ++*log; }
};
constexpr int iter_temps(int x) {
  int r = 0, i = 0, dtors = 0;
  switch (x) {
  case 0:
    for (; i < 4; i += Step{1, &dtors}.v) {
      r += 1;
    case 1:
      r += 10;
    }
  }
  return r * 100 + dtors;
}

// GNU case ranges among the nested labels.
constexpr int ranges(int x) {
  int r = 0, n = 2;
  switch (x) {
  case 0:
    while (n-- > 0) {
      r += 1;
    case 1 ... 3:
      r += 10;
    case 100 ... 200:
      r += 100;
    }
  }
  return r;
}

// Duff's device.
constexpr int duff(int count) {
  int sum = 0, k = 0;
  int n = (count + 3) / 4;
  switch (count % 4) {
  case 0: do { sum += ++k;
  case 3:      sum += ++k * 2;
  case 2:      sum += ++k * 3;
  case 1:      sum += ++k * 4;
          } while (--n > 0);
  }
  return sum;
}

volatile int zero = 0;
#define RT(a) (zero + (a))

template <int CE> bool check(int rt, const char *what) {
  printf("%s = %d\n", what, rt);
  if (rt != CE)
    printf("  mismatch: constexpr %d\n", CE);
  return rt == CE;
}
#define CHECK(ce, rt) ok &= check<ce>(rt, #ce)

int main() {
  bool ok = true;
  CHECK(in_for(0), in_for(RT(0)));
  CHECK(in_for(1), in_for(RT(1)));
  CHECK(in_for(2), in_for(RT(2)));
  CHECK(in_for(3), in_for(RT(3)));
  CHECK(in_for(4), in_for(RT(4)));
  CHECK(in_while(4, 0), in_while(RT(4), RT(0)));
  CHECK(in_while(4, 1), in_while(RT(4), RT(1)));
  CHECK(in_while(4, 2), in_while(RT(4), RT(2)));
  CHECK(in_while(5, 0), in_while(RT(5), RT(0)));
  CHECK(in_while(5, 1), in_while(RT(5), RT(1)));
  CHECK(in_while(5, 2), in_while(RT(5), RT(2)));
  CHECK(in_while(6, 0), in_while(RT(6), RT(0)));
  CHECK(in_while(6, 1), in_while(RT(6), RT(1)));
  CHECK(in_while(6, 2), in_while(RT(6), RT(2)));
  CHECK(in_block(0), in_block(RT(0)));
  CHECK(in_block(1), in_block(RT(1)));
  CHECK(in_block(2), in_block(RT(2)));
  CHECK(in_block(3), in_block(RT(3)));
  CHECK(in_block(4), in_block(RT(4)));
  CHECK(in_block(5), in_block(RT(5)));
  CHECK(in_block(6), in_block(RT(6)));
  CHECK(in_block(7), in_block(RT(7)));
  CHECK(in_block(8), in_block(RT(8)));
  CHECK(in_block(9), in_block(RT(9)));
  CHECK(cont_break(0), cont_break(RT(0)));
  CHECK(cont_break(1), cont_break(RT(1)));
  CHECK(cont_break(2), cont_break(RT(2)));
  CHECK(cont_break(3), cont_break(RT(3)));
  CHECK(cont_break(4), cont_break(RT(4)));
  CHECK(do_continue(0, 1), do_continue(RT(0), RT(1)));
  CHECK(do_continue(0, 2), do_continue(RT(0), RT(2)));
  CHECK(do_continue(0, 3), do_continue(RT(0), RT(3)));
  CHECK(do_continue(0, 4), do_continue(RT(0), RT(4)));
  CHECK(do_continue(1, 1), do_continue(RT(1), RT(1)));
  CHECK(do_continue(1, 2), do_continue(RT(1), RT(2)));
  CHECK(do_continue(1, 3), do_continue(RT(1), RT(3)));
  CHECK(do_continue(1, 4), do_continue(RT(1), RT(4)));
  CHECK(do_continue(2, 1), do_continue(RT(2), RT(1)));
  CHECK(do_continue(2, 2), do_continue(RT(2), RT(2)));
  CHECK(do_continue(2, 3), do_continue(RT(2), RT(3)));
  CHECK(do_continue(2, 4), do_continue(RT(2), RT(4)));
  CHECK(nested_switch(0, 0), nested_switch(RT(0), RT(0)));
  CHECK(nested_switch(0, 1), nested_switch(RT(0), RT(1)));
  CHECK(nested_switch(0, 2), nested_switch(RT(0), RT(2)));
  CHECK(nested_switch(0, 3), nested_switch(RT(0), RT(3)));
  CHECK(nested_switch(1, 0), nested_switch(RT(1), RT(0)));
  CHECK(nested_switch(1, 1), nested_switch(RT(1), RT(1)));
  CHECK(nested_switch(1, 2), nested_switch(RT(1), RT(2)));
  CHECK(nested_switch(1, 3), nested_switch(RT(1), RT(3)));
  CHECK(nested_switch(2, 0), nested_switch(RT(2), RT(0)));
  CHECK(nested_switch(2, 1), nested_switch(RT(2), RT(1)));
  CHECK(nested_switch(2, 2), nested_switch(RT(2), RT(2)));
  CHECK(nested_switch(2, 3), nested_switch(RT(2), RT(3)));
  CHECK(iter_temps(0), iter_temps(RT(0)));
  CHECK(iter_temps(1), iter_temps(RT(1)));
  CHECK(iter_temps(2), iter_temps(RT(2)));
  CHECK(ranges(0), ranges(RT(0)));
  CHECK(ranges(1), ranges(RT(1)));
  CHECK(ranges(2), ranges(RT(2)));
  CHECK(ranges(3), ranges(RT(3)));
  CHECK(ranges(4), ranges(RT(4)));
  CHECK(ranges(100), ranges(RT(100)));
  CHECK(ranges(150), ranges(RT(150)));
  CHECK(ranges(200), ranges(RT(200)));
  CHECK(ranges(201), ranges(RT(201)));
  CHECK(duff(1), duff(RT(1)));
  CHECK(duff(2), duff(RT(2)));
  CHECK(duff(3), duff(RT(3)));
  CHECK(duff(4), duff(RT(4)));
  CHECK(duff(5), duff(RT(5)));
  CHECK(duff(6), duff(RT(6)));
  CHECK(duff(7), duff(RT(7)));
  CHECK(duff(8), duff(RT(8)));
  CHECK(duff(9), duff(RT(9)));
  return ok ? 0 : 1;
}
