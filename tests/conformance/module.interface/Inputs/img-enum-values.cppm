// Enumerator values whose type is a typedef or `bool` for the
// declaration image (spec/modules.md §11.11 "Encoding", WP-1307).
export module imgev;
export namespace ev {
typedef unsigned char u8;
using i16 = short;
enum class Kind : bool { unsized, sized };
enum class Arg : u8 { none, boolean, last = 255 };
enum Wide : i16 { lo = -300, hi = 300 };
template <Kind K> constexpr int pick() { return K == Kind::sized ? 7 : 3; }
constexpr int sum() { return (int)Arg::last + (int)Arg::boolean + hi + lo; }
} // namespace ev
