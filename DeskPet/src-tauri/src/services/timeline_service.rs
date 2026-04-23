use crate::core::ghost::Ghost;
use crate::core::soul::emotional_event::{EmotionalEvent, EmotionalEventType, EmotionalEventManager};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};

const INACTIVITY_WARN_HOURS: i64 = 24;
const INACTIVITY_IGNORE_HOURS: i64 = 72;
const CURIOSITY_CHECK_INTERVAL_SECS: i64 = 300;
const EMOTIONAL_TICK_INTERVAL_SECS: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineState {
    pub last_interaction: DateTime<Utc>,
    pub last_emotional_tick: DateTime<Utc>,
    pub last_curiosity_check: DateTime<Utc>,
    pub last_inactivity_event: DateTime<Utc>,
    pub event_manager: EmotionalEventManager,
}

impl Default for TimelineState {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            last_interaction: now,
            last_emotional_tick: now,
            last_curiosity_check: now,
            last_inactivity_event: now,
            event_manager: EmotionalEventManager::default(),
        }
    }
}

impl TimelineState {
    pub fn record_interaction(&mut self) {
        self.last_interaction = Utc::now();
    }

    pub fn should_emotional_tick(&self) -> bool {
        let elapsed = Utc::now() - self.last_emotional_tick;
        elapsed.num_seconds() >= EMOTIONAL_TICK_INTERVAL_SECS
    }

    pub fn should_check_inactivity(&self) -> bool {
        let since_last_event = Utc::now() - self.last_inactivity_event;
        since_last_event.num_seconds() >= 3600
    }

    pub fn should_check_curiosity(&self) -> bool {
        let elapsed = Utc::now() - self.last_curiosity_check;
        elapsed.num_seconds() >= CURIOSITY_CHECK_INTERVAL_SECS
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineTickResult {
    pub love_hate_after_tick: f64,
    pub baseline_after_tick: f64,
    pub inactivity_event: Option<InactivityEventResult>,
    pub curiosity_triggered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InactivityEventResult {
    pub event_type: String,
    pub intensity: f64,
    pub love_hate_delta: f64,
    pub hours_away: i64,
}

pub struct TimelineService;

impl TimelineService {
    pub fn new() -> Self {
        Self
    }

    pub fn tick(
        ghost: &mut Ghost,
        timeline: &mut TimelineState,
        rng: &mut impl Rng,
    ) -> TimelineTickResult {
        let mut inactivity_event = None;

        if timeline.should_emotional_tick() {
            ghost.soul.sensibility.apply_spring(rng);
            timeline.last_emotional_tick = Utc::now();
        }

        if timeline.should_check_inactivity() {
            if let Some(event) = Self::check_inactivity(ghost, timeline, rng) {
                let hours_away = (Utc::now() - timeline.last_interaction).num_hours();
                inactivity_event = Some(InactivityEventResult {
                    event_type: format!("{:?}", event.event_type),
                    intensity: event.intensity,
                    love_hate_delta: event.love_hate_delta,
                    hours_away,
                });
                timeline.event_manager.add_event(event);
                timeline.last_inactivity_event = Utc::now();
            }
        }

        let curiosity_triggered = if timeline.should_check_curiosity() {
            timeline.last_curiosity_check = Utc::now();
            ghost.soul.curiosity.should_initiate_action()
        } else {
            false
        };

        TimelineTickResult {
            love_hate_after_tick: ghost.soul.sensibility.love_hate,
            baseline_after_tick: ghost.soul.sensibility.baseline,
            inactivity_event,
            curiosity_triggered,
        }
    }

    fn check_inactivity(
        ghost: &mut Ghost,
        timeline: &TimelineState,
        rng: &mut impl Rng,
    ) -> Option<EmotionalEvent> {
        let now = Utc::now();
        let hours_since = (now - timeline.last_interaction).num_hours();

        if hours_since >= INACTIVITY_IGNORE_HOURS {
            let intensity = ((hours_since as f64 / INACTIVITY_IGNORE_HOURS as f64) - 1.0)
                .min(2.0)
                .min(1.0);
            let mut event = EmotionalEvent::new(
                EmotionalEventType::UserIgnoredPet,
                intensity,
            );
            event.description = format!("主人已经{}小时没有和我互动了...", hours_since);
            ghost.soul.apply_emotional_event(&event, rng);
            return Some(event);
        } else if hours_since >= INACTIVITY_WARN_HOURS {
            let intensity = 0.3;
            let mut event = EmotionalEvent::new(
                EmotionalEventType::UserIgnoredPet,
                intensity,
            );
            event.description = format!("主人已经{}小时没来了...", hours_since);
            ghost.soul.apply_emotional_event(&event, rng);
            return Some(event);
        }

        None
    }

    pub fn get_inactivity_status(timeline: &TimelineState) -> InactivityStatus {
        let now = Utc::now();
        let duration = now - timeline.last_interaction;
        InactivityStatus {
            hours_since_last_interaction: duration.num_hours(),
            minutes_since_last_interaction: duration.num_minutes(),
            is_inactive: duration.num_hours() >= INACTIVITY_WARN_HOURS,
            is_very_inactive: duration.num_hours() >= INACTIVITY_IGNORE_HOURS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InactivityStatus {
    pub hours_since_last_interaction: i64,
    pub minutes_since_last_interaction: i64,
    pub is_inactive: bool,
    pub is_very_inactive: bool,
}
