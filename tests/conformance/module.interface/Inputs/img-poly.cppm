// Polymorphic classes for image-poly-run.cpp (spec/modules.md §11.11
// "Classes (S3)", WP-1120): virtual and pure virtual functions,
// overriders, `final`, virtual destructors, a key function defined in
// this unit, a diamond through virtual bases and an inline-only
// polymorphic class. The declaration image carries all of them.
export module imgp;
namespace ip {
export long trace = 0;
// Virtual dispatch; the key function (the destructor, the first
// non-inline virtual function) and `name` are defined in this unit.
export struct Animal {
  virtual ~Animal();
  virtual int legs() const { return 0; }
  virtual const char *name() const;
  int twice() const { return 2 * legs(); }
};
Animal::~Animal() { trace = trace * 10 + 1; }
const char *Animal::name() const { return "animal"; }
export struct Dog : Animal {
  ~Dog() override { trace = trace * 10 + 2; }
  int legs() const override { return 4; }
  const char *name() const override { return "dog"; }
};
export struct Bird final : Animal {
  int legs() const final { return 2; }
};
// An abstract base the importer implements.
export struct Shape {
  virtual ~Shape() = default;
  virtual int area() const = 0;
  int doubled() const { return 2 * area(); }
};
export int area_of(const Shape &s) { return s.area(); }
// A diamond through virtual bases.
export struct Base {
  int b;
  explicit Base(int x) : b(x) {}
  virtual int id() const { return b; }
};
export struct Left : virtual Base {
  Left() : Base(1) {}
  int id() const override { return Base::id() + 10; }
};
export struct Right : virtual Base {
  Right() : Base(2) {}
  virtual int r() const { return b * 3; }
};
export struct Join : Left, Right {
  Join() : Base(7) {}
  int id() const override { return Left::id() + 100; }
};
// An inline-only polymorphic class (constexpr virtuals, no key function).
export struct Counter {
  int n = 0;
  constexpr virtual ~Counter() = default;
  constexpr virtual int step() const { return 1; }
  constexpr int run(int k) { for (int i = 0; i < k; ++i) n += step(); return n; }
};
export struct Double : Counter {
  constexpr int step() const override { return 2; }
};
export constexpr int kRun = Double().run(5);
} // namespace ip
