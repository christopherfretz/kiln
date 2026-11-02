// [stmt.switch]/2, [conv.general]/5: a class-type condition is converted
// once by its conversion function to the switch's integral or enumeration
// type, then promoted; also in constant evaluation.
//@ expect: run
//@ std: c++26
//@ strict-diags
enum class Color { Red, Green, Blue };

int calls = 0;
struct Counted {
  int v;
  operator int() const { ++calls; return v; }
};
struct Small {
  short s;
  constexpr operator const short&() const { return s; }
};
struct Paint {
  Color c;
  constexpr operator Color() const { return c; }
};
struct Base { int v; constexpr operator int() const { return v * 2; } };
struct Derived : Base {};
struct Both {
  int v;
  constexpr operator int() const& { return v; }
  constexpr operator int() && { return v + 100; }
};

constexpr int pick(Small s) {
  switch (s) { case 1: return 10; case 2: return 20; default: return -1; }
}
constexpr int hue(Paint p) {
  switch (p) {
  case Color::Red: return 1;
  case Color::Green: return 2;
  case Color::Blue: return 3;
  }
  return 0;
}
constexpr int base(Derived d) {
  switch (d) { case 6: return 1; default: return 0; }
}
constexpr int both(Both b) {
  int r = 0;
  switch (b) { case 1: r += 1; break; default: break; }
  switch (Both{1}) { case 101: r += 10; break; default: break; }
  return r;
}
static_assert(pick(Small{2}) == 20 && pick(Small{5}) == -1);
static_assert(hue(Paint{Color::Blue}) == 3);
static_assert(base(Derived{{3}}) == 1);
static_assert(both(Both{1}) == 11);

int run(Counted c) {
  switch (c) {
  case 1: return 100;
  case 2: return 200;
  default: return 0;
  }
}

int main() {
  if (run(Counted{2}) != 200 || calls != 1) return 1;
  if (run(Counted{7}) != 0 || calls != 2) return 2;
  Small s{1};
  Paint p{Color::Green};
  Derived d{{3}};
  Both b{1};
  if (pick(s) != 10 || hue(p) != 2 || base(d) != 1 || both(b) != 11) return 3;
  return 0;
}
