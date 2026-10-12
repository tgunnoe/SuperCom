# Contract for the v0 fields of a projectile blueprint body; every other field is
# left open.

{
  Projectile = {
    Categories | Array String | optional,
    Physics
      | {
        InitialSpeed | Number | optional,
        MaxSpeed | Number | optional,
        Acceleration | Number | optional,
        TurnRate | Number | optional,
        Lifetime | Number | optional,
        TrackTarget | Bool | optional,
        UseGravity | Bool | optional,
        LeadTarget | Bool | optional,
        VelocityAlign | Bool | optional,
        DestroyOnWater | Bool | optional,
        ..
      },
    ..
  },
}
