// Debug info (WP-814): while and do-while loops and their loop metadata locations.
//@ args: -g
int count(int n) {
  int i = 0;
  while (i < n) {
    ++i;
  }
  do {
    i--;
  } while (i > 0);
  return i;
}
