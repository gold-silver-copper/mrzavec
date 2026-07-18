use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    Confusion,
    Hallucination,
    SeeInvisible,
    Blindness,
    Levitation,
    Haste,
    MonsterDetection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fuse {
    pub effect: Effect,
    pub turns: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scheduler {
    pub fuses: Vec<Fuse>,
}

impl Scheduler {
    pub fn add_fuse(&mut self, effect: Effect, turns: i32) {
        self.fuses.push(Fuse { effect, turns });
    }

    pub fn add_or_lengthen(&mut self, effect: Effect, turns: i32) {
        if let Some(fuse) = self.fuses.iter_mut().find(|f| f.effect == effect) {
            fuse.turns += turns;
        } else {
            self.fuses.push(Fuse { effect, turns });
        }
    }

    pub fn tick(&mut self) -> Vec<Effect> {
        for fuse in &mut self.fuses {
            fuse.turns -= 1;
        }
        let expired = self
            .fuses
            .iter()
            .filter(|f| f.turns <= 0)
            .map(|f| f.effect)
            .collect();
        self.fuses.retain(|f| f.turns > 0);
        expired
    }

    pub fn cancel(&mut self, effect: Effect) {
        self.fuses.retain(|fuse| fuse.effect != effect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuses_lengthen_and_expire_once() {
        let mut s = Scheduler::default();
        s.add_or_lengthen(Effect::Blindness, 2);
        s.add_or_lengthen(Effect::Blindness, 1);
        assert!(s.tick().is_empty());
        assert!(s.tick().is_empty());
        assert_eq!(s.tick(), vec![Effect::Blindness]);
        assert!(s.tick().is_empty());
    }

    #[test]
    fn independent_fuses_for_the_same_effect_expire_independently() {
        let mut s = Scheduler::default();
        s.add_fuse(Effect::MonsterDetection, 1);
        s.add_fuse(Effect::MonsterDetection, 2);
        assert_eq!(s.tick(), vec![Effect::MonsterDetection]);
        assert_eq!(s.tick(), vec![Effect::MonsterDetection]);
    }
}
