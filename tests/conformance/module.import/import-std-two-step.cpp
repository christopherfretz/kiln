// [std.modules], [module.import] (spec/modules.md §11.11): the two-step
// build of `import std;`. libc++'s std.cppm is precompiled to a module
// file (and its object), then the importer reads that file through
// `-fmodule-file=std=` and links the object; the same commands drive kiln
// and clang. `-Wno-reserved-module-identifier` is what CMake passes for
// the std module (clang warns that `std` is reserved; kiln does not).
//@ expect: run
//@ std: c++23
//@ args: -stdlib=libc++ -Wno-reserved-module-identifier
//@ module: std=%{libcxx_modules}/std.cppm
//@ stdout: 6 abc 3
//@ stdout: 2 4 6
import std;

int main() {
  std::vector<int> v{3, 1, 2};
  std::ranges::sort(v);
  std::string s = "ab";
  s += 'c';
  std::map<std::string, int> m{{"a", 1}, {"b", 2}, {"c", 3}};
  std::println("{} {} {}", std::accumulate(v.begin(), v.end(), 0), s, m.size());
  for (int x : v | std::views::transform([](int i) { return 2 * i; }))
    std::print("{}{}", x, x == 6 ? "\n" : " ");
}
