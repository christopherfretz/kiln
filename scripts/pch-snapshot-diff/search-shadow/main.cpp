#include <only.h>
#include <vector>
int main() {
  std::vector<int> v{A, ONLY, LOCAL, MAYBE};
  return static_cast<int>(v.size()) - 4 + static_cast<int>(name().size()) - 1;
}
