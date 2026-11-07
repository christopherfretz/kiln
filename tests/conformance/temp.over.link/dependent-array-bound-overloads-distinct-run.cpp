// [temp.over.link]/1: function templates that differ only in the element
// type of a parameter of type `const T (&)[N]` are distinct, so each
// specialization is its own entity (abseil LogMessage::operator<< for
// `const char (&)[SIZE]` and `const wchar_t (&)[SIZE]`; both used to be
// one linkonce symbol and the wide literal ran the narrow body).
//@ expect: run
//@ stdout: c3w3w5c5
#include <cstdio>
struct Log {
  template <int N> Log &operator<<(const char (&)[N]) {
    std::printf("c%d", N);
    return *this;
  }
  template <int N> Log &operator<<(const wchar_t (&)[N]) {
    std::printf("w%d", N);
    return *this;
  }
};
int main() {
  Log l;
  l << "ab" << L"ab" << L"wide" << "narr";
  std::printf("\n");
}
