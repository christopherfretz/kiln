// `&typeid(T)` as a constant (WP-176): a constexpr pointer or aggregate
// member initialized with it is emitted statically as the RTTI global,
// as libc++'s `static constexpr __policy` in <functional>.
namespace std {
class type_info {
public:
  virtual ~type_info();
  const char* name() const noexcept { return __name; }
protected:
  const char* __name;
};
}
struct S { virtual ~S(); };
struct E {};
struct P {
  void* (*const clone)(const void*);
  const bool null;
  const std::type_info* const ti;
};
constexpr const std::type_info* q = &typeid(int);
constexpr P p = {nullptr, true, &typeid(void)};
const std::type_info* qs = &typeid(S);
const std::type_info* qe = &typeid(E);
static_assert(&typeid(int) == q);
static_assert(p.ti == &typeid(void));
static_assert(&typeid(E) != &typeid(S));
const P* create_empty() {
  static constexpr P pol = {nullptr, true, &typeid(const E)};
  return &pol;
}
const std::type_info* get() { return q; }
const P* getp() { return &p; }
