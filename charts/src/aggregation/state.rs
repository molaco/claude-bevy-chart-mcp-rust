use super::types::AggregationLevel;
use bevy::prelude::*;

const LEVEL_CHANGE_COOLDOWN_MS: u128 = 200;

#[derive(Resource)]
pub struct AggregationState {
    pub current_level: AggregationLevel,
    pub level_change_cooldown: Option<std::time::Instant>,
}

impl Default for AggregationState {
    fn default() -> Self {
        Self {
            current_level: AggregationLevel::None,
            level_change_cooldown: None,
        }
    }
}

impl AggregationState {
    pub fn should_change_level(&mut self, new_level: AggregationLevel) -> bool {
        if new_level == self.current_level {
            return false;
        }

        // Check cooldown
        if let Some(last_change) = self.level_change_cooldown {
            if last_change.elapsed().as_millis() < LEVEL_CHANGE_COOLDOWN_MS {
                return false;
            }
        }

        // Allow level change
        self.current_level = new_level;
        self.level_change_cooldown = Some(std::time::Instant::now());
        true
    }

    pub fn get_stable_level(
        &mut self,
        visible_count: usize,
        max_renderable: usize,
    ) -> AggregationLevel {
        let new_level = AggregationLevel::select(visible_count, max_renderable);

        if self.should_change_level(new_level) {
            new_level
        } else {
            self.current_level
        }
    }
}
