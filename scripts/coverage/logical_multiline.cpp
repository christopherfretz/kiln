// Coverage comparison program (scripts/coverage-compare.py): `&&`/`||`
// split across lines (the merge branch has no line, the phi line 0).
bool f(int e) {
  const int lo = 2;
  return e >= lo &&
         e <= 3;
}
bool g(const char* it, const char* end) {
  do {
    ++it;
  } while (it != end && (*it == 'a' || ('0' <= *it && *it <= '9')));
  return it == end;
}
int main() {
  int n = 0;
  for (int i = 0; i < 10; i++) n += f(i);
  const char s[] = "ab1c";
  n += g(s, s + 4);
  return n == 2 ? 0 : 1;
}
