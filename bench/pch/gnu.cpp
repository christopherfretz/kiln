// The pch group's `gnu` translation unit (spec/bench.md "The pch group").
// It has no #include of its own: the run passes `-include gnu.h` (no PCH)
// or `-include-pch <pch of gnu.h>`, and the code uses what the header set
// declares, so a consumer that skipped the header could not compile it.

int bench_use(int n) {
  std::vector<int> v;
  for (int i = 0; i < n; ++i)
    v.push_back((i * 7919) % 101);
  std::sort(v.begin(), v.end());
  std::map<std::string, int> m;
  for (int x : v)
    m[std::to_string(x % 13)] += x;
  std::string s;
  for (const auto &[k, x] : m)
    s += k;
  return static_cast<int>(s.size()) + (m.empty() ? 0 : m.begin()->second);
}
