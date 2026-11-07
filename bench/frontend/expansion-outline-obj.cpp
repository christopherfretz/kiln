//@ mode: obj
//@ args: -O3
// One function template whose body is a single `template for` over 400
// constexpr indices ([stmt.expand], P1306; WP-793). Each element runs a
// range-for over a constexpr span of a constexpr table, three linear
// searches through predicate pointers that inline into the element, and
// a string concatenation passed to a may-throw sink, so the expanded
// function holds ~1,600 loops and hundreds of landing pads. As one
// function, `opt-19 default<O3>` takes 69 s and 1.0 GB on it (GVN and
// LoopFullUnroll are superlinear in one function's size). Kiln emits
// each element of an expansion this large as an internal noinline
// helper (`-fkiln-outline-expansions`, spec/landed/codegen/WP-793.md):
// `opt` then takes 1.6 s and 85 MB. kiln only: clang 19/22 lack
// `template for`.
#include <array>
#include <cstddef>
#include <span>
#include <string>
#include <string_view>

namespace syn {

struct Row { int id; int group; bool flag; char kind; const char *name; long pad[40]; };

inline constexpr Row rows[] = {
  {0, 0, true, 0, "row_0", {}},
  {1, 1, false, 1, "row_1", {}},
  {2, 2, false, 2, "row_2", {}},
  {3, 3, false, 3, "row_3", {}},
  {4, 4, false, 4, "row_4", {}},
  {5, 0, false, 5, "row_5", {}},
  {6, 1, false, 6, "row_6", {}},
  {7, 2, true, 7, "row_7", {}},
  {8, 3, false, 8, "row_8", {}},
  {9, 4, false, 9, "row_9", {}},
  {10, 0, false, 10, "row_10", {}},
  {11, 1, false, 11, "row_11", {}},
  {12, 2, false, 12, "row_12", {}},
  {13, 3, false, 0, "row_13", {}},
  {14, 4, true, 1, "row_14", {}},
  {15, 0, false, 2, "row_15", {}},
  {16, 1, false, 3, "row_16", {}},
  {17, 2, false, 4, "row_17", {}},
  {18, 3, false, 5, "row_18", {}},
  {19, 4, false, 6, "row_19", {}},
  {20, 0, false, 7, "row_20", {}},
  {21, 1, true, 8, "row_21", {}},
  {22, 2, false, 9, "row_22", {}},
  {23, 3, false, 10, "row_23", {}},
};

struct Table { std::span<const Row> rows; std::size_t extra; };
inline constexpr Table table{rows, 0};

inline bool is_a(const Row &r) { return r.flag || r.kind == 3; }
inline bool is_b(const Row &r) { return r.kind == 10 || r.kind == 11; }
inline bool is_c(const Row &r) { return r.group == 4 && r.kind != 0; }

inline bool any_of(const Table &t, bool (*p)(const Row &)) {
  for (std::size_t i = 0; i < t.rows.size(); ++i)
    if (p(t.rows[i])) return true;
  return false;
}

[[gnu::noinline]] void emit(std::string &out, std::string_view s); // may throw

template <int N> consteval std::array<int, N> make_idx() {
  std::array<int, N> a{};
  for (int i = 0; i < N; ++i) a[i] = i;
  return a;
}

template <int N> void run(std::string &out) {
  static constexpr auto idx = make_idx<N>();
  template for (constexpr int I : idx) {
    constexpr std::span<const Row> sub{rows + (I % 8), 8};
    for (const Row &r : sub) {
      if (r.id == I) emit(out, r.name);
    }
    if (any_of(table, is_a)) emit(out, "a");
    if (any_of(table, is_b)) emit(out, "b");
    if (any_of(table, is_c)) emit(out, "c");
    std::string s = std::string("prefix_") + rows[I % 24].name;
    emit(out, s);
  }
}

template void run<400>(std::string &);

} // namespace syn
