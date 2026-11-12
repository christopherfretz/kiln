module;
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char *name() const noexcept;
};
} // namespace std
export module imgbf;
export namespace bf {
namespace lib {
inline int f(int x) { return x + 1; }
inline namespace v1 {
inline int g() { return 7; }
} // namespace v1
} // namespace lib
struct P {
  int a;
  constexpr bool operator==(const P &) const = default;
};
constexpr bool ne(P x, P y) { return x != y; }
struct B {
  virtual ~B() {}
  int b = 1;
};
struct D : B {
  int d = 2;
};
inline int dc(B *p) {
  D *q = dynamic_cast<D *>(p);
  return q ? q->d : -1;
}
inline bool same(B &r) { return &typeid(r) == &typeid(D) && &typeid(int) != &typeid(long); }
inline void del(int *p, int *a) {
  delete p;
  delete[] a;
}
inline float bc(unsigned u) { return __builtin_bit_cast(float, u); }
inline int at(int *p) {
  return __atomic_load_n(p, __ATOMIC_SEQ_CST) + __atomic_fetch_add(p, 1, __ATOMIC_RELAXED);
}
struct Base {
  int m() const { return 5; }
};
struct Der : Base {
  int m() const { return Base::m() + this->Base::m(); }
};
inline int lu() {
  using lib::f;
  using namespace lib;
  return f(1) + g();
}
inline int tr(int x) {
  try {
    if (x)
      throw x;
    return 0;
  } catch (int e) {
    return e;
  } catch (...) {
    return -2;
  }
}
enum class Col { red, green };
inline int ue(Col c) {
  using enum Col;
  namespace L = lib;
  return c == green ? L::f(0) : 0;
}
inline int ext() { return int(__array_extent(int[3][4], 1)) + int(__array_rank(int[2][2])); }
union U {
  int i;
  float f;
};
inline int un() {
  U u{5};
  return u.i;
}
struct Pt {
  int x, y;
};
inline int sb() {
  Pt p{3, 4};
  auto [a, b] = p;
  int arr[2] = {5, 6};
  auto &[c, d] = arr;
  return a * b + c + d;
}
template <class T> int tsb(T t) {
  auto [u, v] = t;
  return u - v;
}
inline int usb() { return tsb(Pt{9, 2}); }
consteval Pt mk() { return {1, 2}; }
inline int ag() { return mk().x + mk().y; }
inline constexpr int gv = 4;
consteval const int *gp() { return &gv; }
inline int pc() { return *gp(); }
template <class T> concept Any = true;
inline Any auto cr() { return 3; }
} // namespace bf
