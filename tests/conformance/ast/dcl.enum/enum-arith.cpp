//@ expect: ast
enum Color { Red, Green = 4, Blue };
enum class Mode : unsigned char { Off, On };
int score(Color c, Mode m) {
  int s = c + 1;
  s += Blue - Red;
  if (c == Green) s *= 2;
  return s + (int)m + (int)Mode::On;
}
