// Debug info (WP-814): scoped and unscoped enumerations with fixed and default underlying types.
//@ args: -g
enum class Color { Red, Green = 5 };
enum Old { A, B };
enum class Small : unsigned char { X, Y = 4 };
int f(Color c, Old o, Small s) {
  Color d = Color::Green;
  return (int)c + o + (int)s + (int)d + B;
}
