//@ mode: pp
//@ args: -stdlib=libc++
// Preprocess-only cost of <format>, one of the heaviest libc++ headers.
#include <format>

int main() { return std::format("{}", 42).size() == 2 ? 0 : 1; }
