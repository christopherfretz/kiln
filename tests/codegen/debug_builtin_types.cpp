// Debug info (WP-814): the base types of the arithmetic builtins.
//@ args: -g
int conv(int q) {
  __int128 big = q;
  long double ld = 1.0;
  unsigned short us = 2;
  signed char sc = 3;
  bool flag = q > 0;
  char16_t c16 = u'a';
  wchar_t wc = L'b';
  float fl = 1.5f;
  return (int)big + (int)ld + us + sc + flag + c16 + wc + (int)fl;
}
