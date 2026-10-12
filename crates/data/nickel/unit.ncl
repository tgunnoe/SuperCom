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
        BuildableCategory | Array String | optional,
        MaintenanceConsumptionPerSecondEnergy | Number | optional,
        ..
      },
    Physics
      | {
        MotionType | String,
        MaxSpeed | Number | optional,
        TurnRate | Number | optional,
        MaxAcceleration | Number | optional,
        MaxBrake | Number | optional,
        Elevation | Number | optional,
        MinSpeedPercent | Number | optional,
        BuildRestriction | String | optional,
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
        DamageRadius | Number | optional,
        DamageType | String,
        DamageFriendly | Bool | optional,
        MinRadius | Number | optional,
        MuzzleVelocity | Number | optional,
        ProjectilesPerOnFire | Number | optional,
        MuzzleSalvoSize | Number | optional,
        MuzzleSalvoDelay | Number | optional,
        RackSalvoReloadTime | Number | optional,
        RackSalvoChargeTime | Number | optional,
        RackSalvoFiresAfterCharge | Bool | optional,
        BallisticArc | String | optional,
        TargetPriorities | Array String | optional,
        FireTargetLayerCapsTable | { _ | String } | optional,
        TrackingRadius | Number | optional,
        FiringTolerance | Number | optional,
        TargetCheckInterval | Number | optional,
        WeaponCategory | String,
        Turreted | Bool | optional,
        # A boolean on 13 installed entries and the number 1 on 9.
        ManualFire | std.contract.from_predicate (fun v => v == true || v == false || v == 1) | optional,
        ..
      }
      | optional,
    General
      | {
        CommandCaps | { _ | Bool },
        ..
      },
    Footprint
      | {
        SizeX | Number | optional,
        SizeZ | Number | optional,
        MinWaterDepth | Number | optional,
        ..
      }
      | optional,
    ..
  },
}
