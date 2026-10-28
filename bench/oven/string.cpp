//@ args: -stdlib=libc++ -fconstexpr-steps=200000000
// String building in constant evaluation with std::string: transient
// heap allocation, reallocation on growth, appends, and integer-to-text
// conversion. Exercises constexpr new/delete and allocator traffic.
#include <cstddef>
#include <string>
#include <string_view>

constexpr std::string to_dec(unsigned v) {
  if (v == 0) return "0";
  std::string s;
  while (v) {
    s.insert(s.begin(), static_cast<char>('0' + v % 10));
    v /= 10;
  }
  return s;
}

// "1,2,3,...,n" built by repeated append (amortized growth).
constexpr std::string join(unsigned n) {
  std::string out;
  for (unsigned i = 1; i <= n; ++i) {
    if (i > 1) out += ',';
    out += to_dec(i);
  }
  return out;
}

constexpr std::size_t count(std::string_view s, char c) {
  std::size_t k = 0;
  for (char x : s) k += (x == c);
  return k;
}

// FizzBuzz text, the classic generated-string benchmark.
constexpr std::string fizzbuzz(unsigned n) {
  std::string out;
  for (unsigned i = 1; i <= n; ++i) {
    if (i % 15 == 0) out += "FizzBuzz";
    else if (i % 3 == 0) out += "Fizz";
    else if (i % 5 == 0) out += "Buzz";
    else out += to_dec(i);
    out += '\n';
  }
  return out;
}

static_assert(to_dec(1234567) == "1234567");
static_assert(join(12) == "1,2,3,4,5,6,7,8,9,10,11,12");
static_assert(join(600).size() == 9 + 2 * 90 + 3 * 501 + 599);
static_assert(count(join(600), ',') == 599);
static_assert(count(fizzbuzz(800), '\n') == 800);
static_assert(fizzbuzz(15).ends_with("14\nFizzBuzz\n"));
