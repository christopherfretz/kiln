// [re.alg.match], [re.alg.search], [re.regex]: std::regex (ECMAScript and
// POSIX extended grammars), std::regex_match and std::regex_search compile
// and run against libc++ (the libstdc++ sibling is
// regex-match-libstdcxx.cpp; WP-608).
//@ expect: run
//@ args: -stdlib=libc++
//@ strict-diags
//@ stdout: 1 0 1 bob example 1
#include <cstdio>
#include <regex>

int main() {
  std::regex re("a+b");
  bool m1 = std::regex_match("aab", re);
  bool m2 = std::regex_match("abb", re);
  std::regex mail("(\\w+)@(\\w+)\\.com", std::regex::ECMAScript);
  std::cmatch m;
  bool s = std::regex_search("mail bob@example.com now", m, mail);
  std::regex digits("[[:digit:]]+", std::regex::extended);
  bool m3 = std::regex_match("123", digits);
  std::printf("%d %d %d %s %s %d\n", m1, m2, s, s ? m[1].str().c_str() : "-",
              s ? m[2].str().c_str() : "-", m3);
  if (!m1 || m2)
    return 1;
  if (!s || m[1].str() != "bob" || m[2].str() != "example")
    return 2;
  return m3 ? 0 : 3;
}
