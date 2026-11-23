// The prefix of pch.cpp and pch-off.cpp, precompiled with the
// written-reference table on (pch.cpp) or off (pch-off.cpp).
#define PCH_DECL(n) int n = 0;
namespace pch_ns {
PCH_DECL(from_pch)
struct Pt {
  int x;
  int get() const { return x; }
};
int norm(Pt p) { return p.get() + from_pch; }
template <class T> int measure(T t) { return norm(t) + t.get(); }
template <class T> struct Box {
  T value;
  T get() const { return value; }
};
} // namespace pch_ns
namespace outer {
using namespace pch_ns;
inline int twice(Pt p) { return norm(p) * 2; }
} // namespace outer
// A leading `::` and a destructor's `~` are carried (WP-2326).
inline void lead(pch_ns::Pt *p) { p->::pch_ns::Pt::~Pt(); }
// Operator entries with their closing tokens, and ud-suffixes, are
// carried (WP-2820).
namespace pch_ns {
struct Grid {
  int operator[](int i) const { return i; }
};
inline Pt operator+(Pt a, Pt b) { return {a.x + b.x}; }
inline int operator""_px(unsigned long long v) { return int(v); }
inline int cell(Grid g, Pt p) { return g[(p + p).x] + 3_px; }
} // namespace pch_ns
