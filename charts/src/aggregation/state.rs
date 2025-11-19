use super::types::AggregationLevel;
use bevy::prelude::*;

const LEVEL_CHANGE_COOLDOWN_MS: u128 = 200;

#[derive(Resource)]
pub struct AggregationState {
    pub current_level: AggregationLevel,
    pub previous_level: AggregationLevel,
    pub level_change_cooldown: Option<std::time::Instant>,
}

impl Default for AggregationState {
    fn default() -> Self {
        Self {
            current_level: AggregationLevel::None,
            previous_level: AggregationLevel::None,
            level_change_cooldown: None,
        }
    }
}

impl AggregationState {
    pub fn should_change_level(&mut self, new_level: AggregationLevel, visible_count: usize) -> bool {
        if new_level == self.current_level {
            return false;
        }

        // Check cooldown
        if let Some(last_change) = self.level_change_cooldown {
            if last_change.elapsed().as_millis() < LEVEL_CHANGE_COOLDOWN_MS {
                return false;
            }
        }

        // Hysteresis: only change if we've crossed the threshold with sufficient margin
        // This prevents rapid flickering when zooming near threshold boundaries
        let should_transition = match (self.current_level, new_level) {
            // Going up in aggregation (more candles visible)
            // Use 110% of threshold to create upward buffer
            (AggregationLevel::None, AggregationLevel::Low) => visible_count > 1650,
            (AggregationLevel::Low, AggregationLevel::Medium) => visible_count > 3300,
            (AggregationLevel::Medium, AggregationLevel::High) => visible_count > 6600,
            (AggregationLevel::High, AggregationLevel::VeryHigh) => visible_count > 13200,
            (AggregationLevel::VeryHigh, AggregationLevel::Extreme) => visible_count > 26400,
            (AggregationLevel::Extreme, AggregationLevel::Maximum) => visible_count > 52800,

            // Going down in aggregation (fewer candles visible)
            // Use 90% of threshold to create downward buffer
            (AggregationLevel::Low, AggregationLevel::None) => visible_count < 1350,
            (AggregationLevel::Medium, AggregationLevel::Low) => visible_count < 2700,
            (AggregationLevel::High, AggregationLevel::Medium) => visible_count < 5400,
            (AggregationLevel::VeryHigh, AggregationLevel::High) => visible_count < 10800,
            (AggregationLevel::Extreme, AggregationLevel::VeryHigh) => visible_count < 21600,
            (AggregationLevel::Maximum, AggregationLevel::Extreme) => visible_count < 43200,

            // Large jumps (e.g., None → Medium) - allow immediately
            _ => true,
        };

        if !should_transition {
            return false;
        }

        // Allow level change - track previous level before updating
        self.previous_level = self.current_level;
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

        if self.should_change_level(new_level, visible_count) {
            new_level
        } else {
            self.current_level
        }
    }
}
