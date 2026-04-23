use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmotionalEventType {
    UserInitiatedChat,
    UserCaredAboutPet,
    UserPraisedPet,
    UserSharedPersonalStory,
    UserCelebratedTogether,
    FirstConversation,
    BirthdayCelebrated,
    UserIgnoredPet,
    UserGotAngry,
    UserDismissedPet,
    NormalChat,
    CuriosityTriggered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalEvent {
    pub event_type: EmotionalEventType,
    pub intensity: f64,
    pub love_hate_delta: f64,
    pub baseline_delta: f64,
    pub timestamp: DateTime<Utc>,
    pub description: String,
}

impl EmotionalEvent {
    pub fn new(event_type: EmotionalEventType, intensity: f64) -> Self {
        let (love_hate_delta, baseline_delta) = Self::calculate_deltas(&event_type, intensity);
        Self {
            event_type,
            intensity: intensity.clamp(0.0, 1.0),
            love_hate_delta,
            baseline_delta,
            timestamp: Utc::now(),
            description: String::new(),
        }
    }

    pub fn new_with_description(
        event_type: EmotionalEventType,
        intensity: f64,
        description: String,
    ) -> Self {
        let (love_hate_delta, baseline_delta) = Self::calculate_deltas(&event_type, intensity);
        Self {
            event_type,
            intensity: intensity.clamp(0.0, 1.0),
            love_hate_delta,
            baseline_delta,
            timestamp: Utc::now(),
            description,
        }
    }

    fn calculate_deltas(event_type: &EmotionalEventType, intensity: f64) -> (f64, f64) {
        match event_type {
            EmotionalEventType::UserInitiatedChat => (5.0 * intensity, 0.0),
            EmotionalEventType::UserCaredAboutPet => (8.0 * intensity, 0.0),
            EmotionalEventType::UserPraisedPet => (6.0 * intensity, 0.0),
            EmotionalEventType::UserSharedPersonalStory => (10.0 * intensity, 2.0 * intensity),
            EmotionalEventType::UserCelebratedTogether => (15.0 * intensity, 5.0 * intensity),
            EmotionalEventType::FirstConversation => (15.0, 5.0),
            EmotionalEventType::BirthdayCelebrated => (20.0, 8.0),
            EmotionalEventType::UserIgnoredPet => (-5.0 * intensity, 0.0),
            EmotionalEventType::UserGotAngry => (-15.0 * intensity, 0.0),
            EmotionalEventType::UserDismissedPet => (-8.0 * intensity, 0.0),
            EmotionalEventType::NormalChat => (2.0 * intensity, 0.0),
            EmotionalEventType::CuriosityTriggered => (1.0, 0.0),
        }
    }

    pub fn is_milestone(&self) -> bool {
        matches!(
            self.event_type,
            EmotionalEventType::FirstConversation
                | EmotionalEventType::BirthdayCelebrated
                | EmotionalEventType::UserCelebratedTogether
        )
    }

    pub fn is_positive(&self) -> bool {
        self.love_hate_delta > 0.0
    }

    pub fn is_negative(&self) -> bool {
        self.love_hate_delta < 0.0
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmotionalEventManager {
    pub events: Vec<EmotionalEvent>,
}

impl EmotionalEventManager {
    pub fn add_event(&mut self, event: EmotionalEvent) {
        self.events.push(event);
        while self.events.len() > 100 {
            self.events.remove(0);
        }
    }

    pub fn get_recent_events(&self, count: usize) -> &[EmotionalEvent] {
        let start = self.events.len().saturating_sub(count);
        &self.events[start..]
    }

    pub fn should_trigger_ignore_event(
        &self,
        last_interaction: DateTime<Utc>,
    ) -> Option<EmotionalEvent> {
        let now = Utc::now();
        let duration = now - last_interaction;
        if duration.num_hours() >= 72 {
            Some(EmotionalEvent::new(
                EmotionalEventType::UserIgnoredPet,
                (duration.num_hours() as f64 / 72.0).min(1.0),
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_deltas() {
        let event = EmotionalEvent::new(EmotionalEventType::FirstConversation, 1.0);
        assert_eq!(event.love_hate_delta, 15.0);
        assert_eq!(event.baseline_delta, 5.0);
        assert!(event.is_milestone());
        assert!(event.is_positive());
    }

    #[test]
    fn test_negative_event() {
        let event = EmotionalEvent::new(EmotionalEventType::UserGotAngry, 0.5);
        assert!(event.love_hate_delta < 0.0);
        assert!(event.is_negative());
        assert!(!event.is_milestone());
    }

    #[test]
    fn test_event_manager() {
        let mut manager = EmotionalEventManager::default();
        for _i in 0..5 {
            manager.add_event(EmotionalEvent::new(EmotionalEventType::NormalChat, 0.5));
        }
        assert_eq!(manager.events.len(), 5);
    }
}
