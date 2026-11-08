// [temp.mem]/6, [class.conv.fct]: a specialization of a conversion function template is not
// found by name lookup; `e.operator Matx<double, 1, 2>()` considers each conversion function
// template of the object's class (not of its bases, as clang) and uses the specialization
// deduced for the named type ([temp.deduct.conv]). `this->Mat::operator Matx<T, m, n>()` in a class
// template derived from `Mat` names the base's template the same way (OpenCV's
// `Mat_<_Tp>::operator Matx`; WP-803).
//@ expect: run
//@ strict-diags
//@ stdout: 5 8 6 1
#include <cstdio>

template <class T, int m, int n> struct Matx { T v[m * n]; };

struct Mat {
    double d = 2;
    template <class T, int m, int n> operator Matx<T, m, n>() const {
        Matx<T, m, n> r{};
        r.v[0] = T(d) + m + n;
        return r;
    }
    operator int() const { return 1; }
};

template <class T> struct Mat_ : Mat {
    template <int m, int n> operator Matx<T, m, n>() const {
        Matx<T, m, n> r = this->Mat::operator Matx<T, m, n>();
        return r;
    }
};

int main() {
    Mat a;
    Matx<double, 1, 2> x = a.operator Matx<double, 1, 2>();
    Mat_<double> b;
    Matx<double, 3, 3> y = b;
    Matx<int, 2, 2> z = a.operator Matx<int, 2, 2>();
    std::printf("%d %d %d %d\n", int(x.v[0]), int(y.v[0]), z.v[0], a.operator int());
}
