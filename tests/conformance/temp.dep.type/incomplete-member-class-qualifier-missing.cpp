//@ verify
// WP-726: the lookup of `Block::v` waits for instantiation, where a missing
// member is diagnosed in the specialization (as clang).
template <class I> struct Q {
  struct Block;
  int g() { return Block::v; } // expected-error {{no member named 'v' in 'Q<int>::Block'}}
};
template <class I> struct Q<I>::Block { static const int w = 1; };
int main() { Q<int> q; return q.g(); } // expected-note {{in instantiation of member function 'Q<int>::g' requested here}}
