// [std.modules], [module.import]: `import std;` under C++20 builds libc++'s
// std.cppm (and its partitions) in-process; std::vector, std::string and
// algorithms are usable, and the exported `using` declarations name the
// library's entities.
//@ expect: run
//@ std: c++20
//@ args: -stdlib=libc++
//@ stdout: 6 abc 3
//@ skip(clang): clang needs a prebuilt std BMI (two-step --precompile of libc++'s std.cppm) that the harness cannot drive
import std;

int main() {
  std::vector<int> v{3, 1, 2};
  std::sort(v.begin(), v.end());
  int sum = std::accumulate(v.begin(), v.end(), 0);
  std::string s = "ab";
  s += 'c';
  std::printf("%d %s %zu\n", sum, s.c_str(), s.size());
  return v.front() == 1 && v.back() == 3 ? 0 : 1;
}
