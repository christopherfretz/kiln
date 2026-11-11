// Polymorphic class templates and pack-expansion base specifiers for the
// declaration image (spec/modules.md §11.11 "Polymorphic class templates
// and pack bases", WP-1292).
export module imgpoly;
export namespace pt {
struct Shape {
  virtual ~Shape() {}
  virtual int area() const = 0;
  virtual int sides() const { return 0; }
};
// A class template deriving from a polymorphic class: overriders, a final
// class and a final overrider.
template <class T> struct Sq final : Shape {
  T s;
  int area() const override { return (int)(s * s); }
  int sides() const final { return 4; }
};
// A polymorphic class template with a pure virtual function and a
// defaulted virtual destructor, and a class template overriding it.
template <class T> struct Abs {
  virtual T f() const = 0;
  virtual ~Abs() = default;
};
template <class T> struct Impl : Abs<T> {
  T f() const override { return T(11); }
};
// A pack expansion in a base-specifier-list ([temp.variadic]/5.2).
template <class... Ts> struct Many : Ts... {
  int n() const { return sizeof...(Ts); }
};
struct G1 { int one() const { return 1; } };
struct G2 { int two() const { return 2; } };
template <class T> int tid(const Shape &s) { return s.area() + (int)sizeof(T); }
} // namespace pt
