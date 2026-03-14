pub mod navigation;
pub mod progression;

use bevy::prelude::*;

pub struct SystemsPlugin;

impl Plugin for SystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((navigation::NavigationPlugin, progression::ProgressionPlugin));
    }
}
