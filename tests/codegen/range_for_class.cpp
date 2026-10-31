// range-based for over classes: member begin/end, ADL begin/end, an
// iterator class with operator!=/++/*, a range temporary with a destructor
// (extended by binding to __range), and std::initializer_list.
namespace std {
template <class E> class initializer_list {
  const E* b;
  decltype(sizeof 0) n;
  constexpr initializer_list(const E* p, decltype(sizeof 0) c) : b(p), n(c) {}

public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr const E* begin() const { return b; }
  constexpr const E* end() const { return b + n; }
};
}  // namespace std

struct Vec {
  int d[4];
  int* begin() { return d; }
  int* end() { return d + 4; }
};
namespace N {
struct Bag { short v[3]; };
short* begin(Bag& b) { return b.v; }
short* end(Bag& b) { return b.v + 3; }
}  // namespace N
struct It {
  int i;
  bool operator!=(const It& o) const { return i != o.i; }
  It& operator++() { ++i; return *this; }
  int operator*() const { return i * i; }
};
struct Count {
  int n;
  It begin() const { return {0}; }
  It end() const { return {n}; }
};
struct Owner {
  int d[2];
  ~Owner();
  const int* begin() const { return d; }
  const int* end() const { return d + 2; }
};
Owner make();

int members(Vec& v) {
  int s = 0;
  for (int x : v) s += x;
  return s;
}
int adl(N::Bag b) {
  int s = 0;
  for (auto x : b) s += x;
  return s;
}
int squares(int n) {
  int s = 0;
  for (int x : Count{n}) s += x;
  return s;
}
int owned() {
  int s = 0;
  for (int x : make()) s += x;
  return s;
}
int list() {
  int s = 0;
  for (int x : {1, 2, 3}) s += x;
  return s;
}
