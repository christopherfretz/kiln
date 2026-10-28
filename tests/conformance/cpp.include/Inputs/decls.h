// Declarations made through macros defined in this header.
#define DECLARE_COUNTER(name, init) inline constexpr int name = init;
#define SQUARE(x) ((x) * (x))
#define FIELD(type, name) type name;
DECLARE_COUNTER(header_counter, 7)
struct Point {
  FIELD(int, x)
  FIELD(int, y)
};
constexpr int header_square(int v) { return SQUARE(v); }
