// [basic.start.main]: the standard input stream is the program's stdin.
//@ expect: run
//@ run-stdin-file: Inputs/numbers.txt
//@ stdout: 12
//@ xfail(kiln): libstdc++ headers do not parse yet: function overloading (c++config.h) and templates
#include <iostream>

int main() {
  int sum = 0;
  for (int n; std::cin >> n;) sum += n;
  std::cout << sum << '\n';
}
