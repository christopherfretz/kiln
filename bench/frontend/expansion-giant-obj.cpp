//@ mode: obj
//@ args: -O3
// One function template whose body is a single `template for` over 48
// constexpr indices ([stmt.expand], P1306; WP-783). Every iteration builds
// an ostringstream, a std::set<std::string> and string temporaries, calls
// two may-throw helpers, and runs small loops over constexpr arrays, so the
// expanded function holds hundreds of invokes, cleanup landing pads and
// lifetime-marked allocas, and a dozen-odd loops per iteration. The cost is
// LLVM's `default<O3>` on one wide function (LoopFullUnroll, LoopRotate,
// SROA lead), not kiln: the same body expanded by a macro gives kiln and
// clang 19 the same pre-opt IR shape (blocks, invokes, landing pads,
// memsets, lifetime markers within 1%) and the same `opt` time
// (spec/codegen.md "Wide expansion statements"). kiln only: clang 19/22 lack
// `template for`; g++ 16.1 compiles it in about the same time.
#include <array>
#include <set>
#include <sstream>
#include <string>
#include <string_view>

namespace wide {

[[gnu::noinline]] std::string name_of(int i);            // may throw
[[gnu::noinline]] void sink(std::string_view s, int i);  // may throw

inline constexpr std::array<int, 4> small_table{3, 1, 4, 1};
inline constexpr std::array<std::string_view, 3> words{"alpha", "beta", "gamma"};

template <int N> consteval std::array<int, N> make_idx() {
  std::array<int, N> a{};
  for (int i = 0; i < N; ++i) a[i] = 100 + i;
  return a;
}

template <int N> void run(std::string &out) {
  static constexpr auto idx = make_idx<N>();
  template for (constexpr int I : idx) {
    std::ostringstream os;
    std::set<std::string> seen;
    std::string a = name_of(I);
    std::string b = a + "_" + std::to_string(I);
    seen.insert(a);
    seen.insert(b);
    for (int v : small_table) os << v << ',';
    if constexpr (I % 3 == 0) {
      for (std::string_view w : words) seen.insert(std::string(w));
    }
    os << b << seen.size();
    std::string_view sv = b;
    if (sv.size() > 3) sink(os.str(), I);
    out += os.str();
  }
}

template void run<48>(std::string &);

} // namespace wide
