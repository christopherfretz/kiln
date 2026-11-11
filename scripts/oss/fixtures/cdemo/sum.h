#ifndef CDEMO_SUM_H
#define CDEMO_SUM_H
#ifdef __cplusplus
extern "C" {
#endif
struct point { int x, y; };
int sum_points(const struct point *p, int n);
#ifdef __cplusplus
}
#endif
#endif
