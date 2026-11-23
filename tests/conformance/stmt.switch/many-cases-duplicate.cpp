// [stmt.switch]/2: the case values of a switch are distinct, also far
// apart in a long run of case labels and ranges; each duplicate names
// the earlier case, for a range the largest value it overlaps (the
// duplicate check is hashed since WP-2478).
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-gnu-case-range
//@ diag: 20:8 error "duplicate case value '3'"
//@ diag: 14:32 note "previous case defined here"
//@ diag: 21:8 error "duplicate case value '63'"
//@ diag: 17:143 note "previous case defined here"
int f(int x) {
  switch (x) {
  case 0: case 1: case 2: case 3: case 4: case 5: case 6: case 7: case 8: case 9: case 10: case 11: case 12: case 13: case 14: case 15:
  case 16: case 17: case 18: case 19: case 20: case 21: case 22: case 23: case 24: case 25: case 26: case 27: case 28: case 29: case 30: case 31:
  case 32: case 33: case 34: case 35: case 36: case 37: case 38: case 39: case 40: case 41: case 42: case 43: case 44: case 45: case 46: case 47:
  case 48: case 49: case 50: case 51: case 52: case 53: case 54: case 55: case 56: case 57: case 58: case 59: case 60: case 61: case 62: case 63:
  case 100 ... 200:
    return 1;
  case 3:
  case 60 ... 70:
    return 2;
  }
  return 0;
}
