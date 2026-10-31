// Key functions: a strong vtable (key function defined here), an external
// one (key function defined elsewhere), an abstract class with a pure
// virtual, devirtualized calls (final, object of known type).
// Virtual and devirtualized delete; noexcept throughout (no EH before WP-100).
struct Shape {
  virtual ~Shape();
  virtual int area() const noexcept = 0;
  virtual int sides() const noexcept { return 0; }
};
Shape::~Shape() {}

struct Ext {
  virtual void key() noexcept;
  virtual int v() noexcept { return 3; }
};

struct Sq final : Shape {
  int s;
  Sq(int s) noexcept : s(s) {}
  int area() const noexcept override { return s * s; }
  int sides() const noexcept override { return 4; }
};

struct Tri : Shape {
  int b, h;
  Tri(int b, int h) noexcept : b(b), h(h) {}
  int area() const noexcept override { return b * h / 2; }
  int sides() const noexcept final { return 3; }
};

int use_ext(Ext* e) noexcept { return e->v(); }
int sum(const Shape& s) noexcept { return s.area() + s.sides(); }
int direct(Sq* q, Tri* t) noexcept { return q->area() + t->sides(); }
int local() noexcept {
  Tri t(4, 6);
  Sq q(3);
  return t.area() + q.area() + sum(t) + sum(q) + direct(&q, &t);
}
void del(Shape* s) noexcept { delete s; }
void del_final(Sq* s) noexcept { delete s; }
