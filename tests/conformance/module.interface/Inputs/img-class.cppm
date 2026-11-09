// A class-heavy module interface for image-class-run.cpp
// (spec/modules.md §11.11 "Part 2a", stage S3): non-template classes
// with fields, bit-fields, default member initializers, bases,
// constructors (delegating, defaulted), a destructor, conversion and
// operator functions, static data members, nested types, access, a
// final class and a union.
export module imgc;
namespace ic {
export struct Point {
  int x = 0, y = 0;
  constexpr Point() = default;
  constexpr Point(int a, int b) : x(a), y(b) {}
  constexpr Point operator+(const Point &o) const { return {x + o.x, y + o.y}; }
  constexpr bool operator==(const Point &o) const { return x == o.x && y == o.y; }
  constexpr int norm1() const { return (x < 0 ? -x : x) + (y < 0 ? -y : y); }
  static constexpr int kDims = 2;
  static int created;
};
int Point::created = 0;

export struct Flags {
  unsigned a : 3;
  unsigned b : 5 = 7;
  mutable int hits = 0;
  constexpr Flags() : a(1) {}
  constexpr unsigned sum() const { ++hits; return a + b; }
};

export class Counter {
public:
  using value_type = long;
  enum Mode { Up, Down };
  struct Step { value_type by = 1; };
  explicit constexpr Counter(Mode m) : mode(m) {}
  constexpr Counter() : Counter(Up) {}
  constexpr Counter &tick(Step s) {
    n += mode == Up ? s.by : -s.by;
    return *this;
  }
  constexpr Counter &tick() { return tick(Step{}); }
  constexpr value_type get() const { return n; }
  constexpr explicit operator bool() const { return n != 0; }
  int report() const;
private:
  Mode mode;
  value_type n = 0;
};
int Counter::report() const { return int(n) * 2; }

export struct Base1 { int b1 = 1; constexpr int who() const { return b1; } };
export struct Base2 { int b2 = 2; constexpr int two() const { return b2 * 2; } };
export struct Both : Base1, Base2 {
  int own = 3;
  constexpr Both() = default;
  constexpr Both(int a, int b, int c) : Base1{a}, Base2{b}, own(c) {}
  constexpr int total() const { return who() + two() + own + b2; }
};
export constexpr const Base2 &as_base2(const Both &b) { return b; }
export constexpr int base2_of(const Both &b) { return as_base2(b).b2; }

export long trace = 0;
export struct Noisy {
  int id;
  explicit Noisy(int i) : id(i) { trace = trace * 10 + id; }
  Noisy(const Noisy &o) : id(o.id + 100) { trace = trace * 10 + 9; }
  ~Noisy() { trace = trace * 10 + id % 10; }
  Noisy plus(int d) const { return Noisy(id + d); }
};
export inline int noisy_sum(int a) { return Noisy(a).plus(1).id + Noisy(a + 5).id; }

export struct Leaf final : Base1 {
  constexpr int leaf() const { return b1 + 10; }
};

export union Word {
  unsigned u;
  float f;
  constexpr Word() : u(0) {}
};

export struct Wrapper {
  Point p{1, 2};
  Counter c;
  constexpr int score() const { return p.norm1() + int(c.get()); }
};

export constexpr Point kOrigin{};
export constexpr Point kDiag = Point(3, 4) + Point(1, 1);
export constexpr int kTotal = Both(4, 5, 6).total();
export constexpr long kTicks = Counter().tick().tick(Counter::Step{5}).get();
export constexpr int kScore = Wrapper{}.score();
} // namespace ic
