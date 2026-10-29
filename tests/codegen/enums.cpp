// Unscoped and scoped enums, with fixed underlying types.
enum Color { Red, Green = 5, Blue };
enum class Mode : unsigned char { Off, On, Auto = 200 };
enum Big : long { Huge = 1L << 40 };
int color(Color c) { return c + 1; }
Mode next(Mode m) { return m == Mode::Off ? Mode::On : Mode::Auto; }
long big(Big b) { return b; }
int enums(int i) { Color c = (Color)i; return color(c) + (int)next(Mode::Off) + (int)(big(Huge) >> 40) + (c == Blue); }
