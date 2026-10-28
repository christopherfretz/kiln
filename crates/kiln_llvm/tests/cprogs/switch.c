// switch with fallthrough and default.
int classify(int c) {
  switch (c) {
  case 1: return 10;
  case 2:
  case 3: c = 7; break;
  case -5: return 1;
  default: c = 0;
  }
  return c;
}
