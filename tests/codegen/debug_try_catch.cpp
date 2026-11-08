// Debug info (WP-814): try/catch: the catch parameter, the landing pad's artificial location.
//@ args: -g
int may(int v);
int run(int v) {
  int s = 0;
  try {
    s += may(v);
  } catch (int e) {
    s -= e;
  } catch (...) {
  }
  return s;
}
