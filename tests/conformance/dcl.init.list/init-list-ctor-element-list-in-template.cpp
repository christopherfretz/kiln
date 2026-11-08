// [dcl.init.list]/3.7: `V v{"2", 1}` calls V's initializer-list
// constructor with a std::initializer_list over the two elements. The
// element type R also has an initializer-list constructor, so the
// instantiation of a template must rebuild the list from its elements:
// reading the constructor's one argument back as a list element would
// give `V v{{"2", 1}}`, one R holding two (Boost.JSON `value b{"2", 1}`
// became the object {"2":1} in a function template, WP-804). An empty
// list is still the empty std::initializer_list (`V h{}`: 0 elements).
//@ expect: run
//@ strict-diags
//@ stdout: 2:1,3 2:1,3 2:1,3 1:12 2:1,3 1:3 0: |

#include <cstdio>
#include <initializer_list>

struct R {
  R(const char*) : k(1) {}
  R(int) : k(3) {}
  R(std::initializer_list<R> t) : k(10 + (int)t.size()) {}
  int k;
};

struct P {};

struct V {
  V(std::initializer_list<R> init, P = {}) {
    std::printf("%d:", (int)init.size());
    const char* sep = "";
    for (const R& r : init) {
      std::printf("%s%d", sep, r.k);
      sep = ",";
    }
    std::printf(" ");
  }
};

template <class T> void f() {
  V a{"2", 1};
  V b = {"2", 1};
  V c{R("2"), R(1)};
  V d{{"2", 1}};
  V e{"2", T(1)};
  V g{1};
  V h{};
}

int main() {
  f<int>();
  std::printf("|\n");
}
