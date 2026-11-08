// [temp.expl.spec]/1: `template<>` before the out-of-line definition of a constructor (or
// destructor) of an implicitly instantiated class template specialization introduces that one
// declaration. The block-scope variables in its body are not under the head, so `int i(n * 8);`
// declares a plain local, not an extraneous variable specialization (OpenCV's flann
// `LshTable<unsigned char>::LshTable`; WP-803).
//@ expect: run
//@ strict-diags
//@ stdout: 18 2 16
#include <cstdio>
#include <vector>

template <typename T> struct L {
    L(unsigned n);
    ~L();
    int m;
    std::vector<int> v;
};

template <> inline L<char>::L(unsigned n) {
    int i(n * 8);
    m = 2 + i;
    std::vector<int> indices(n * 8);
    v.push_back(int(n));
    v.push_back(int(indices.size()));
}

template <> inline L<char>::~L() {
    int last = v.back();
    std::printf("%d %d %d\n", m, v[0], last);
}

int main() { L<char> l(2); }
