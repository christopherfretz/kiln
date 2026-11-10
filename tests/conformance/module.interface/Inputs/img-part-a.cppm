// An interface partition of imgp (spec/modules.md §11.11 stage S6): its
// image follows its replayed module declaration.
export module imgp:a;
export namespace pa {
struct Pt {
  int x, y;
};
constexpr int norm(Pt p) { return p.x * p.x + p.y * p.y; }
using Coord = int;
} // namespace pa
namespace pa {
// Module linkage: visible to the other units of imgp only.
constexpr int scale = 3;
} // namespace pa
