# Contract for the v0 fields of a prop blueprint body; every other field is
# left open.
let categories = import "categories.ncl" in

{
  Prop = {
    Categories | Array String,
    ..
  },
}
