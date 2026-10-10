# Contract for the v0 fields of a projectile blueprint body; every other field is
# left open.
let categories = import "categories.ncl" in

{
  Projectile = {
    Categories | Array String | optional,
    ..
  },
}
