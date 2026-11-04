// [temp.explicit]/4, /10, /12, [temp.deduct.decl]: an explicit
// instantiation naming a constructor, conversion function, member or free
// function template specialization by its signature deduces the template
// arguments from the declarator (a defaulted non-type template parameter
// from its default, [temp.param]/12). The definition form emits the
// specialization `weak_odr` in a comdat (every structor variant); the
// `extern` form suppresses emission, so a use references it externally
// (`declare`), as clang does at -O0, even for an inline template. WP-580.
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <class T, class U> constexpr bool is_same_v = false;
template <class T> constexpr bool is_same_v<T, T> = true;

struct Str {
  int n;
};

struct Cord {
  template <typename T, typename enable_if<is_same_v<T, Str>, int>::type = 0>
  explicit Cord(T &&src);
  template <class T> operator T *() const;
  template <class T, int N = 3> int get(T) const;
  template <class T> static int make(T);
  template <class T> Cord(T, T);
  int n = 0;
};
template <typename T, typename enable_if<is_same_v<T, Str>, int>::type>
Cord::Cord(T &&src) : n(src.n) {}
template <class T> Cord::operator T *() const { return nullptr; }
template <class T, int N> int Cord::get(T) const { return n + N; }
template <class T> int Cord::make(T t) { return int(t); }
template <class T> inline Cord::Cord(T, T) {}

template Cord::Cord(Str &&);
template Cord::operator char *() const;
template int Cord::get(long) const;
template int Cord::make(short);

extern template Cord::Cord(int, int);
extern template int Cord::get(int) const;

template <class T> T twice(T t) { return t + t; }
template int twice(int);
template long twice<>(long);
template short twice<short>(short);
extern template char twice(char);

int use() {
  Cord c(1, 2);
  return c.get(0) + twice('a');
}
