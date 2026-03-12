pub mod items;
pub mod anomalies;
pub mod buoyancy;
pub mod boss;
pub mod water_crystals;

use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            items::ItemPlugin,
            anomalies::AnomalyPlugin,
            buoyancy::BuoyancyPlugin,
            boss::BossPlugin,
            water_crystals::WaterCrystalPlugin,
        ));
    }
}
