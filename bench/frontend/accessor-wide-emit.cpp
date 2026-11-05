//@ mode: emit-llvm
// One class with 400 data members and 1200 accessor bindings through a
// class-template `add` chain in one registration function, each binding a
// lambda wrapper whose call thunk holds an argument converter per
// parameter in a `std::tuple`; the converter of the class itself holds
// the class by value (WP-633). Codegen's fold prepass used to evaluate
// every automatic aggregate's initializer, so each thunk's converter
// tuple built the whole 400-member object in the Oven, once per binding:
// cubic in the member count (3.3 s here, 70 s at 1600 members; 2.0 s and
// 6.5 s now). Only a POD or `const` automatic aggregate is folded now
// (clang `IsConstantAggregate`).
#include <cstddef>
#include <string>
#include <tuple>
#include <type_traits>
#include <utility>
#include <vector>

namespace reg {
struct slot { void *p = nullptr; };
struct box { long count; void *kind; };
void release(box *) noexcept;
inline void retain(void *q) noexcept { auto *o = static_cast<box *>(q); if (o->count >= 0) ++o->count; }
inline void drop(void *q) noexcept { auto *o = static_cast<box *>(q); if (o->count >= 0 && --o->count == 0) release(o); }
struct ref {
  void *p = nullptr;
  ref() = default;
  explicit ref(void *q) : p(q) {}
  ref(const ref &o) : p(o.p) { if (p) retain(p); }
  ref(ref &&o) noexcept : p(o.p) { o.p = nullptr; }
  ref &operator=(const ref &o) { ref t(o); std::swap(p, t.p); return *this; }
  ~ref() { if (p) drop(p); }
};
struct param_info { const char *name; void *value; unsigned char flags; long extra[3]; };
struct named {
  const char *name;
  ref value;
  explicit named(const char *n) : name(n) {}
};
struct entry {
  const char *name = nullptr;
  void *data[3] = {};
  slot (*thunk)(void *, slot *) = nullptr;
  void (*cleanup)(entry *) = nullptr;
  std::size_t nargs = 0;
  param_info params[4] = {};
  ref owner;
};
box *publish(const entry *, const char *const *names);
[[noreturn]] void fail();
void attach(const ref &owner, const char *name, const ref &value);

template <class T> struct conv {
  std::remove_cvref_t<T> value{};
  bool take(slot s) { return s.p != nullptr; }
  static slot give(const T &) { return slot{}; }
  operator T &() { return value; }
};
template <> struct conv<std::string> {
  std::string value;
  bool take(slot s) { if (!s.p) return false; value.assign(static_cast<const char *>(s.p)); return true; }
  static slot give(const std::string &s) { return slot{const_cast<char *>(s.data())}; }
  operator std::string &() { return value; }
};
template <class T> struct conv<std::vector<T>> {
  std::vector<T> value;
  bool take(slot s) { if (!s.p) return false; value.resize(4); return true; }
  static slot give(const std::vector<T> &v) { return slot{const_cast<T *>(v.data())}; }
  operator std::vector<T> &() { return value; }
};
template <class T> using conv_for = conv<std::remove_cvref_t<T>>;

template <class... Args> struct names_of {
  static constexpr const char *names[sizeof...(Args) + 1] = {"self", (sizeof(Args) ? "arg" : "")...};
};

template <class Ret, class... Args, class Func, std::size_t... Is>
slot invoke(Func &f, slot *in, std::index_sequence<Is...>) {
  std::tuple<conv_for<Args>...> convs;
  if ((!std::get<Is>(convs).take(in[Is]) || ...)) return slot{};
  if constexpr (std::is_void_v<Ret>) {
    f(static_cast<Args>(std::get<Is>(convs))...);
    return slot{};
  } else {
    return conv_for<Ret>::give(f(static_cast<Args>(std::get<Is>(convs))...));
  }
}

inline void note(entry &e, std::size_t &i, const named &a) {
  param_info d{a.name, a.value.p, 1, {1, 2, 3}};
  if (a.value.p) retain(a.value.p);
  e.params[i++] = d;
}
template <class Func, class Ret, class... Args, class... Extra>
[[gnu::always_inline]] inline ref wrap(Func &&f, Ret (*)(Args...), const char *name, const ref &owner, const Extra &...extra) {
  using F = std::remove_reference_t<Func>;
  entry e;
  e.name = name;
  e.nargs = sizeof...(Args);
  e.owner = owner;
  std::size_t i = 0;
  (note(e, i, extra), ...);
  if constexpr (sizeof(F) <= sizeof(e.data)) {
    new (static_cast<void *>(e.data)) F(std::forward<Func>(f));
  } else {
    e.data[0] = new F(std::forward<Func>(f));
    e.cleanup = [](entry *r) { delete static_cast<F *>(r->data[0]); };
  }
  e.thunk = [](void *p, slot *in) -> slot {
    F *fp = sizeof(F) <= sizeof(entry::data) ? static_cast<F *>(p) : *static_cast<F **>(p);
    return invoke<Ret, Args...>(*fp, in, std::index_sequence_for<Args...>{});
  };
  entry copy = e;
  box *b = publish(&copy, names_of<Args...>::names);
  if (!b) fail();
  return ref(b);
}

template <class T> class klass : public ref {
public:
  explicit klass(const char *name) : name_(name) {}
  template <class C, class R, class... A>
  klass &add(const char *n, R (C::*pm)(A...), const auto &...extra) {
    ref fn = wrap([pm](T *self, A... a) -> R { return (self->*pm)(std::forward<A>(a)...); },
                  static_cast<R (*)(T *, A...)>(nullptr), n, *this, extra...);
    attach(*this, n, fn);
    return *this;
  }
  template <class C, class R, class... A>
  klass &add(const char *n, R (C::*pm)(A...) const, const auto &...extra) {
    ref fn = wrap([pm](const T *self, A... a) -> R { return (self->*pm)(std::forward<A>(a)...); },
                  static_cast<R (*)(const T *, A...)>(nullptr), n, *this, extra...);
    attach(*this, n, fn);
    return *this;
  }
  template <class G, class S>
  klass &add_prop(const char *n, G &&g, S &&s) {
    using V = decltype(g(std::declval<const T &>()));
    ref fg = wrap(std::forward<G>(g), static_cast<V (*)(const T &)>(nullptr), n, *this);
    ref fs = wrap(std::forward<S>(s), static_cast<void (*)(T &, V)>(nullptr), n, *this, named("value"));
    attach(*this, n, fg);
    attach(*this, n, fs);
    return *this;
  }
private:
  const char *name_;
};
} // namespace reg

#define TY0 int
#define TY1 double
#define TY2 std::string
#define TY3 std::vector<int>
#define MEMBER(i, t) \
  t m_##i{}; \
  const t &get_##i() const { return m_##i; } \
  void set_##i(const t &v) { m_##i = v; }
#define M4(i) MEMBER(i##0, TY0) MEMBER(i##1, TY1) MEMBER(i##2, TY2) MEMBER(i##3, TY3)
#define M20(i) M4(i##0) M4(i##1) M4(i##2) M4(i##3) M4(i##4)
#define M100(i) M20(i##0) M20(i##1) M20(i##2) M20(i##3) M20(i##4)
#define BIND(i, t) \
  .add("get_" #i, &Wide::get_##i) \
  .add("set_" #i, &Wide::set_##i, reg::named("v")) \
  .add_prop(#i, [](const Wide &w) -> t { return w.get_##i(); }, [](Wide &w, t v) { w.set_##i(v); })
#define B4(i) BIND(i##0, TY0) BIND(i##1, TY1) BIND(i##2, TY2) BIND(i##3, TY3)
#define B20(i) B4(i##0) B4(i##1) B4(i##2) B4(i##3) B4(i##4)
#define B100(i) B20(i##0) B20(i##1) B20(i##2) B20(i##3) B20(i##4)

struct Wide { M100(a) M100(b) M100(c) M100(d) };

void register_wide() { reg::klass<Wide>("Wide") B100(a) B100(b) B100(c) B100(d); }
