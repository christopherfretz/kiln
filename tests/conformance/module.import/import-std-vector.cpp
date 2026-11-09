// [std.modules], [module.import]: `import std;` with no module flags.
// Kiln finds libc++'s std.cppm itself (spec/modules.md §11.12: its module
// cache, else the in-process build); std::vector, std::string and
// algorithms are usable, and the exported `using` declarations name the
// library's entities. import-std-two-step.cpp is the portable form.
//@ expect: run
//@ std: c++20
//@ args: -stdlib=libc++
//@ stdout: 6 abc 3
//@ skip(clang): clang finds no std module without -fmodule-file=std= (kiln's implicit lookup is what this tests)
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
