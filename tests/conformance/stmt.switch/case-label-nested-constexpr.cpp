// [stmt.switch]/6: a case or default label may label any statement in the
// switch body, also one nested in another statement; the jump transfers
// control into it. Duff's device and its relatives in constant evaluation
// ([expr.const]; C++14 lifted the single-return rule).

// Duff's device: case labels inside a do-while body.
constexpr int duff(int n) {
  int count = 0;
  int k = (n + 3) / 4;
  switch (n % 4) {
  case 0:
    do {
      ++count;
    case 3:
      ++count;
    case 2:
      ++count;
    case 1:
      ++count;
    } while (--k > 0);
  }
  return count;
}
static_assert(duff(1) == 1);
static_assert(duff(7) == 7);
static_assert(duff(8) == 8);
static_assert(duff(13) == 13);

// A label inside the branches of an if statement: the jump skips the
// condition, and falling off the then-branch skips the else-branch.
constexpr int branches(int n) {
  int r = 0;
  switch (n) {
  case 0:
    if (r == 0) {
    case 1:
      r += 10;
    } else {
    case 2:
      r += 100;
    }
    r += 1;
    break;
  default:
    r = -1;
  }
  return r;
}
static_assert(branches(0) == 11);
static_assert(branches(1) == 11);
static_assert(branches(2) == 101);
static_assert(branches(3) == -1);

// Labels inside while and for bodies: the jump skips the first test of the
// condition; later iterations test it as usual.
constexpr int loops(int n) {
  int i = 0, s = 0;
  switch (n) {
    while (i < 3) {
    case 0:
      s += 1;
      ++i;
    }
    break;
  case 1:
    for (; i < 5; ++i) {
    default:
      s += 10;
    }
  }
  return s * 100 + i;
}
static_assert(loops(0) == 303);
static_assert(loops(1) == 5005);
static_assert(loops(7) == 5005);

// A label in a nested block and on a labeled statement.
constexpr int nested(int n) {
  int s = 0;
  switch (n) {
    {
      {
      case 4:
        s += 4;
      }
    lbl:
    case 5:
      s += 5;
    }
  }
  return s;
}
static_assert(nested(4) == 9);
static_assert(nested(5) == 5);
static_assert(nested(6) == 0);
