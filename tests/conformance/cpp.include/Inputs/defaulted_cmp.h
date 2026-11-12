namespace dc {
struct po { int v; friend constexpr bool operator==(po, po) noexcept = default; };
struct tc { int p; long q; friend bool operator==(const tc&, const tc&) = default; };
struct mm { int a; bool operator==(const mm&) const = default; };
template <class T> struct box { T v; friend constexpr bool operator==(const box&, const box&) = default; };
template <class T> struct mbox { T v; constexpr bool operator==(const mbox&) const = default; };
constexpr bool used = po{1} == po{1};
}
namespace o { struct W { int x; }; }
namespace dc2 {
inline bool operator==(o::W a, o::W b) { return a.x == b.x; }
struct U { o::W w; bool operator==(const U&) const = default; };
}
