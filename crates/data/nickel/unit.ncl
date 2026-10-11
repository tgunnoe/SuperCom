# Contract for the v0 fields of a unit blueprint body. Every field outside
# the v0 set is left open. Optional fields are optional exactly where the
# installed data omits them (specification: data model).
let categories = import "categories.ncl" in

{
  Unit = {
    Categories | Array categories.Category,
    Economy
      | {
        BuildCostEnergy | Number,
        BuildCostMass | Number,
        BuildTime | Number | optional,
        BuildRate | Number | optional,
        ProductionPerSecondEnergy | Number | optional,
        ProductionPerSecondMass | Number | optional,
        StorageEnergy | Number | optional,
        StorageMass | Number | optional,
        ..
      },
    Physics
      | {
        MotionType | String,
        MaxSpeed | Number | optional,
        TurnRate | Number | optional,
        ..
      },
    Defense
      | {
        Health | Number,
        MaxHealth | Number,
        ArmorType | String,
        RegenRate | Number | optional,
        ..
      },
    Intel
      | {
        VisionRadius | Number | optional,
        RadarRadius | Number | optional,
        ..
      }
      | optional,
    Weapon
      | Array {
        Label | String | optional,
        Damage | Number,
        RateOfFire | Number | optional,
        MaxRadius | Number | optional,
        ProjectileId | String | optional,
        ..
      }
      | optional,
    ..
  },
}
