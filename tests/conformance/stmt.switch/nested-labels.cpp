// [stmt.switch]/2,5, [stmt.label], [expr.const] (WP-1144): a `case` or
// `default` label may be anywhere in the switch body, including inside
// nested loops, blocks and `if` branches, and the constant evaluator jumps
// to it: the enclosing loops then run their increment, condition,
// `continue` and `break` as usual on the following iterations. The values
// are the program's run-time results (clang++-19); nested-labels-run.cpp
// compares the two at run time.
//@ strict-diags
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

static_assert(in_for(0) == 3333);
static_assert(in_for(1) == 3323);
static_assert(in_for(2) == 3223);
static_assert(in_for(3) == 0);
static_assert(in_for(4) == 0);
static_assert(in_while(4,0) == 0);
static_assert(in_while(4,1) == 3);
static_assert(in_while(4,2) == 12);
static_assert(in_while(5,0) == 2);
static_assert(in_while(5,1) == 2);
static_assert(in_while(5,2) == 9);
static_assert(in_while(6,0) == 0);
static_assert(in_while(6,1) == 3);
static_assert(in_while(6,2) == 12);
static_assert(in_block(0) == 0);
static_assert(in_block(1) == 54031);
static_assert(in_block(2) == 54020);
static_assert(in_block(3) == 50300);
static_assert(in_block(4) == 54000);
static_assert(in_block(5) == 0);
static_assert(in_block(6) == 0);
static_assert(in_block(7) == 0);
static_assert(in_block(8) == 0);
static_assert(in_block(9) == 50000);
static_assert(cont_break(0) == 341906);
static_assert(cont_break(1) == 341906);
static_assert(cont_break(2) == 331906);
static_assert(cont_break(3) == -100);
static_assert(cont_break(4) == 0);
static_assert(do_continue(0,1) == 11);
static_assert(do_continue(0,2) == 22);
static_assert(do_continue(0,3) == 23);
static_assert(do_continue(0,4) == 34);
static_assert(do_continue(1,1) == 10);
static_assert(do_continue(1,2) == 21);
static_assert(do_continue(1,3) == 22);
static_assert(do_continue(1,4) == 33);
static_assert(do_continue(2,1) == 0);
static_assert(do_continue(2,2) == 0);
static_assert(do_continue(2,3) == 0);
static_assert(do_continue(2,4) == 0);
static_assert(nested_switch(0,0) == 44);
static_assert(nested_switch(0,1) == 33);
static_assert(nested_switch(0,2) == 18);
static_assert(nested_switch(0,3) == 14);
static_assert(nested_switch(1,0) == 43);
static_assert(nested_switch(1,1) == 28);
static_assert(nested_switch(1,2) == 10);
static_assert(nested_switch(1,3) == 10);
static_assert(nested_switch(2,0) == 0);
static_assert(nested_switch(2,1) == 0);
static_assert(nested_switch(2,2) == 0);
static_assert(nested_switch(2,3) == 0);
static_assert(iter_temps(0) == 4404);
static_assert(iter_temps(1) == 4304);
static_assert(iter_temps(2) == 0);
static_assert(ranges(0) == 222);
static_assert(ranges(1) == 332);
static_assert(ranges(2) == 332);
static_assert(ranges(3) == 332);
static_assert(ranges(4) == 0);
static_assert(ranges(100) == 322);
static_assert(ranges(150) == 322);
static_assert(ranges(200) == 322);
static_assert(ranges(201) == 0);
static_assert(duff(1) == 4);
static_assert(duff(2) == 11);
static_assert(duff(3) == 20);
static_assert(duff(4) == 30);
static_assert(duff(5) == 44);
static_assert(duff(6) == 61);
static_assert(duff(7) == 80);
static_assert(duff(8) == 100);
static_assert(duff(9) == 124);
