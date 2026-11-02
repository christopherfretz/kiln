// [temp.spec.partial.member]/1, [temp.class.general]/3, [temp.over.link]/6:
// an out-of-line definition `template<class V> requires fw<V> template<bool B>
// class C<V>::It` names the members of the partial specialization
// `template<class V> requires fw<V> class C<V>` even though its argument list
// `<V>` is the primary's own: the enclosing template head's requires-clause is
// equivalent to the partial specialization's, not the primary's (libstdc++
// ranges:6380 `chunk_view<_Vp>::_Iterator`; docs/status/libstdcxx.md, WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 3
extern "C" int printf(const char *, ...);
template<class V> concept in = true;
template<class V> concept fw = in<V> && sizeof(V) > 1;

template<class V> requires in<V> class C {
public:
  static int f() { return 1; }
};

template<class V> requires fw<V> class C<V> {
public:
  template<bool B> class It;
  static int f();
};

template<class V> requires fw<V> template<bool B> class C<V>::It {
public:
  static int g() { return B ? 3 : 0; }
};

template<class V> requires fw<V> int C<V>::f() { return 2; }

int main() {
  printf("%d %d %d\n", C<char>::f(), C<long long>::f(),
         C<long long>::It<true>::g());
}
