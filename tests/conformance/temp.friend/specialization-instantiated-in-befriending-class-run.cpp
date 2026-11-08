// [class.friend]/2, [temp.friend]/1, [temp.inst]/1: `V` befriends the specialization
// `VTraits<V>`, and a later member declaration of `V` names `VTraits<V>::lane_type`, which
// instantiates `VTraits<V>` while `V` is still being defined. The instantiation reads `V`'s
// private `nlanes` and `lane_type`; the friendship already holds there, so the access is
// valid. The befriended function template specialization `get0<V>` reads the private `get()`
// (OpenCV's `VTraits` in `intrin_neon.hpp`; WP-803).
//@ expect: run
//@ strict-diags
//@ stdout: 7 16 16
#include <cstdio>

template <typename T> struct VTraits {
    static int vlanes() { return T::nlanes; }
    enum { max_nlanes = T::nlanes, nlanes = T::nlanes };
    using lane_type = typename T::lane_type;
};

template <typename T> typename VTraits<T>::lane_type get0(const T& v) { return v.get(); }

struct V {
    unsigned char val;

private:
    friend struct VTraits<V>;
    enum { nlanes = 16 };
    typedef unsigned char lane_type;
    friend typename VTraits<V>::lane_type get0<V>(const V& v);
    unsigned char get() const { return val; }
};

int main() {
    V v{7};
    std::printf("%d %d %d\n", get0(v), VTraits<V>::vlanes(), int(VTraits<V>::max_nlanes));
}
