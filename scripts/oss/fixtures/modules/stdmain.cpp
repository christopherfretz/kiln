// `import std;` through CMake's CXX_MODULE_STD (WP-984).
import std;

int main() {
  std::vector<int> v{1, 2, 3};
  std::string s = "import std";
  std::println("{} {}", s, std::accumulate(v.begin(), v.end(), 0));
}
