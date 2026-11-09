// Polymorphic classes for image-virtual-run.cpp (spec/modules.md §11.11
// "Part 2a"): the declaration image does not carry them yet, so the
// unit is written without one and the importer replays its source.
export module imgv;
namespace iv {
export struct Shape {
  constexpr virtual ~Shape() = default;
  constexpr virtual int area() const = 0;
  constexpr virtual int sides() const { return 0; }
  constexpr int twice() const { return 2 * area(); }
};
export struct Square : Shape {
  int s;
  constexpr explicit Square(int n) : s(n) {}
  constexpr int area() const override { return s * s; }
  constexpr int sides() const override { return 4; }
};
export struct Tri final : Shape {
  int b, h;
  constexpr Tri(int x, int y) : b(x), h(y) {}
  constexpr int area() const override { return b * h / 2; }
  constexpr int sides() const final { return 3; }
};
export constexpr int area_of(const Shape &s) { return s.area(); }
export constexpr int kSq = area_of(Square(5));
export inline int dyn_sides(const Shape *s) { return s->sides(); }
} // namespace iv
