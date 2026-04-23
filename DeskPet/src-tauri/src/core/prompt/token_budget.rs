pub struct TokenBudget {
    pub total_budget: usize,
    pub system_reserve: usize,
    pub identity_reserve: usize,
    pub personality_reserve: usize,
    pub emotion_reserve: usize,
    pub experience_reserve: usize,
    pub constraint_reserve: usize,
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self {
            total_budget: 8000,
            system_reserve: 500,
            identity_reserve: 100,
            personality_reserve: 300,
            emotion_reserve: 100,
            experience_reserve: 100,
            constraint_reserve: 200,
        }
    }
}

impl TokenBudget {
    pub fn memory_budget(&self) -> usize {
        self.total_budget
            - self.system_reserve
            - self.identity_reserve
            - self.personality_reserve
            - self.emotion_reserve
            - self.experience_reserve
            - self.constraint_reserve
    }

    pub fn output_budget(&self) -> usize {
        4000
    }

    pub fn core_memory_reserve(&self) -> usize {
        500
    }

    pub fn recent_conversation_reserve(&self) -> usize {
        600
    }

    pub fn fillable_memory_budget(&self) -> usize {
        self.memory_budget()
            - self.core_memory_reserve()
            - self.recent_conversation_reserve()
    }

    pub fn total_system_tokens(&self) -> usize {
        self.system_reserve
            + self.identity_reserve
            + self.personality_reserve
            + self.emotion_reserve
            + self.experience_reserve
            + self.constraint_reserve
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPriority {
    CoreLongTerm = 0,
    RecentConversation = 1,
    HighImportanceLongTerm = 2,
    ExperienceInstruction = 3,
    PersonalityDetail = 4,
    ShortTermHighAccessibility = 5,
    ShortTermLowAccessibility = 6,
}

impl MemoryPriority {
    pub fn from_importance_and_type(importance: f64, is_core: bool, is_short_term: bool, accessibility: f64) -> Self {
        if is_core {
            return MemoryPriority::CoreLongTerm;
        }
        if !is_short_term && importance >= 0.6 {
            return MemoryPriority::HighImportanceLongTerm;
        }
        if is_short_term && accessibility >= 0.3 {
            return MemoryPriority::ShortTermHighAccessibility;
        }
        if is_short_term {
            return MemoryPriority::ShortTermLowAccessibility;
        }
        MemoryPriority::HighImportanceLongTerm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_budget() {
        let budget = TokenBudget::default();
        assert_eq!(budget.total_budget, 8000);
        assert!(budget.memory_budget() > 0);
        assert!(budget.memory_budget() < budget.total_budget);
        assert!(budget.fillable_memory_budget() > 0);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(MemoryPriority::CoreLongTerm < MemoryPriority::RecentConversation);
        assert!(MemoryPriority::RecentConversation < MemoryPriority::HighImportanceLongTerm);
        assert!(MemoryPriority::ShortTermHighAccessibility < MemoryPriority::ShortTermLowAccessibility);
    }
}