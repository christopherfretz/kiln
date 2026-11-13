// The pch group's `meta` translation unit (spec/bench.md "The pch group").
// It has no #include of its own: the run passes `-include meta.h` (no PCH)
// or `-include-pch <pch of meta.h>`. It reflects over its own types with
// the standard <meta> queries, so a consumer that skipped the header could
// not compile it.

struct Sample {
  int id;
  double weight;
  std::string label;
};

enum class Level { low, mid, high };

template <class T> consteval std::size_t field_count() {
  return std::meta::nonstatic_data_members_of(^^T, std::meta::access_context::unchecked()).size();
}

template <class E> consteval std::array<std::string_view, 3> enumerator_names() {
  std::array<std::string_view, 3> out{};
  std::size_t i = 0;
  for (auto e : std::meta::enumerators_of(^^E))
    if (i < out.size())
      out[i++] = std::meta::identifier_of(e);
  return out;
}

static_assert(field_count<Sample>() == 3);
static_assert(enumerator_names<Level>()[2] == "high");
static_assert(std::meta::identifier_of(^^Sample) == "Sample");

int bench_use(int n) {
  constexpr auto names = enumerator_names<Level>();
  std::vector<Sample> v;
  for (int i = 0; i < n; ++i)
    v.push_back({i, i * 0.5, std::string(names[static_cast<std::size_t>(i % 3)])});
  std::sort(v.begin(), v.end(), [](const Sample &a, const Sample &b) { return a.label < b.label; });
  std::map<std::string, int> m;
  for (const Sample &s : v)
    m[s.label] += s.id;
  return static_cast<int>(m.size() + field_count<Sample>());
}
