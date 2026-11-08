// Debug info (WP-814): a switch over an enumeration.
//@ args: -g
enum class Mode : unsigned char { Off, On = 4 };
int sel(Mode m) {
  switch (m) {
  case Mode::Off:
    return 1;
  default:
    break;
  }
  return 0;
}
