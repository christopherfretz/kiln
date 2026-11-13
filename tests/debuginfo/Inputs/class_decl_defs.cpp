// The defining unit of class_decl_two_units.cpp: `Widget` is required
// complete here (a variable and member access), so limited -g describes
// it in full.
#include "class_decl_widget.h"

static gadgets::Widget the_widget{3, 4};

gadgets::Widget *make_widget() { return &the_widget; }

int widget_sum(gadgets::Widget *w) { return w->a + w->b; }
