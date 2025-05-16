//! # Timer
//!
//! This module contains everything related to the timer. A timer can
//! be identified by a state (running or stopped), a cycle and a
//! cycles count (infinite or finite). During the lifetime of the
//! timer, timer events are triggered.

#[cfg(test)]
use mock_instant::Instant;
#[cfg(not(test))]
use std::time::Instant;
use std::{
    io::{Error, ErrorKind, Result},
    ops::{Deref, DerefMut},
};

use serde::{Deserialize, Serialize};

/// The timer loop.
///
/// When the timer reaches its last cycle, it starts again from the
/// first cycle. This structure defines the number of loops the timer
/// should do before stopping by itself.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimerLoop {
    /// The timer loops indefinitely and therefore never stops by
    /// itself.
    ///
    /// The only way to stop such timer is via a stop request.
    #[default]
    Infinite,

    /// The timer stops by itself after the given number of loops.
    Fixed(usize),
}

impl From<usize> for TimerLoop {
    fn from(count: usize) -> Self {
        if count == 0 {
            Self::Infinite
        } else {
            Self::Fixed(count)
        }
    }
}

/// The timer cycle.
///
/// A cycle is a step in the timer lifetime, represented by a name and
/// a duration.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TimerCycle {
    /// The name of the timer cycle.
    pub name: String,

    /// The duration of the timer cycle.
    ///
    /// This field has two meanings, depending on where it is
    /// used. *From the config point of view*, the duration represents
    /// the total duration of the cycle. *From the timer point of
    /// view*, the duration represents the amount of time remaining
    /// before the cycle ends.
    pub duration: usize,
}

impl TimerCycle {
    pub fn new(name: impl ToString, duration: usize) -> Self {
        Self {
            name: name.to_string(),
            duration,
        }
    }
}

impl<T: ToString> From<(T, usize)> for TimerCycle {
    fn from((name, duration): (T, usize)) -> Self {
        Self::new(name, duration)
    }
}

/// The timer cycles list.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TimerCycles(Vec<TimerCycle>);

impl<T: IntoIterator<Item = TimerCycle>> From<T> for TimerCycles {
    fn from(cycles: T) -> Self {
        Self(cycles.into_iter().collect())
    }
}

impl Deref for TimerCycles {
    type Target = Vec<TimerCycle>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TimerCycles {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The timer state.
///
/// Enumeration of all the possible state of a timer: running, paused
/// or stopped.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimerState {
    /// The timer is running.
    Running,

    /// The timer has been paused.
    Paused,

    /// The timer is not running.
    #[default]
    Stopped,
}

/// The timer event.
///
/// Enumeration of all possible events that can be triggered during
/// the timer lifecycle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimerEvent {
    /// The timer started.
    Started,

    /// The timer began the given cycle.
    Began(TimerCycle),

    /// The timer is running the given cycle (tick).
    Running(TimerCycle),

    /// The timer has been set to the given cycle.
    Set(TimerCycle),

    /// The timer has been paused at the given cycle.
    Paused(TimerCycle),

    /// The timer has been resumed at the given cycle.
    Resumed(TimerCycle),

    /// The timer ended with the given cycle.
    Ended(TimerCycle),

    /// The timer stopped.
    Stopped,
}

/// The timer configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TimerConfig {
    /// The list of custom timer cycles.
    pub cycles: TimerCycles,

    /// The timer cycles counter.
    pub cycles_count: TimerLoop,
}

impl TimerConfig {
    fn clone_first_cycle(&self) -> Result<TimerCycle> {
        self.cycles.first().cloned().ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                "cannot find first cycle from timer config",
            )
        })
    }
}

/// The main timer struct.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Timer {
    /// The current timer configuration.
    pub config: TimerConfig,

    /// The current timer state.
    pub state: TimerState,

    /// The current timer cycle.
    pub cycle: TimerCycle,

    /// The current cycles counter.
    pub cycles_count: TimerLoop,

    #[serde(skip)]
    pub started_at: Option<Instant>,

    pub elapsed: usize,
}

impl Timer {
    pub fn new(config: TimerConfig) -> Self {
        let cycle = config.clone_first_cycle().unwrap();
        let cycles_count = config.cycles_count.clone();

        Self {
            config,
            cycle,
            cycles_count,
            ..Default::default()
        }
    }

    pub fn elapsed(&self) -> usize {
        self.started_at
            .map(|i| i.elapsed().as_secs() as usize)
            .unwrap_or_default()
            + self.elapsed
    }

    pub fn update(&mut self) -> impl IntoIterator<Item = TimerEvent> {
        let mut events = Vec::with_capacity(3);

        if let TimerState::Running = self.state {
            let mut elapsed = self.elapsed();

            let (cycles, total_duration) = self.config.cycles.iter().cloned().fold(
                (Vec::new(), 0),
                |(mut cycles, mut sum), mut cycle| {
                    cycle.duration += sum;
                    sum = cycle.duration;
                    cycles.push(cycle);
                    (cycles, sum)
                },
            );

            if let TimerLoop::Fixed(cycles_count) = self.cycles_count {
                if elapsed >= (total_duration * cycles_count) {
                    self.state = TimerState::Stopped;
                    return events;
                }
            }

            elapsed %= total_duration;

            let last_cycle = cycles[cycles.len() - 1].clone();
            let next_cycle = cycles
                .into_iter()
                .fold(None, |next_cycle, mut cycle| match next_cycle {
                    None if elapsed < cycle.duration => {
                        cycle.duration -= elapsed;
                        Some(cycle)
                    }
                    _ => next_cycle,
                })
                .unwrap_or(last_cycle);

            events.push(TimerEvent::Running(self.cycle.clone()));

            if self.cycle.name != next_cycle.name {
                let mut prev_cycle = self.cycle.clone();
                prev_cycle.duration = 0;
                events.push(TimerEvent::Ended(prev_cycle));
                events.push(TimerEvent::Began(next_cycle.clone()));
            }

            self.cycle = next_cycle;
        }

        events
    }

    pub fn start(&mut self) -> impl IntoIterator<Item = TimerEvent> {
        let mut events = Vec::with_capacity(2);

        if matches!(self.state, TimerState::Stopped) {
            self.state = TimerState::Running;
            self.cycle = self.config.clone_first_cycle().unwrap();
            self.cycles_count = self.config.cycles_count.clone();
            self.started_at = Some(Instant::now());
            self.elapsed = 0;
            events.push(TimerEvent::Started);
            events.push(TimerEvent::Began(self.cycle.clone()));
        }

        events
    }

    pub fn set(&mut self, duration: usize) -> impl IntoIterator<Item = TimerEvent> {
        self.cycle.duration = duration;
        Some(TimerEvent::Set(self.cycle.clone()))
    }

    pub fn pause(&mut self) -> impl IntoIterator<Item = TimerEvent> {
        if matches!(self.state, TimerState::Running) {
            self.state = TimerState::Paused;
            self.elapsed = self.elapsed();
            self.started_at = None;
            Some(TimerEvent::Paused(self.cycle.clone()))
        } else {
            None
        }
    }

    pub fn resume(&mut self) -> impl IntoIterator<Item = TimerEvent> {
        if matches!(self.state, TimerState::Paused) {
            self.state = TimerState::Running;
            self.started_at = Some(Instant::now());
            Some(TimerEvent::Resumed(self.cycle.clone()))
        } else {
            None
        }
    }

    pub fn stop(&mut self) -> impl IntoIterator<Item = TimerEvent> {
        let mut events = Vec::with_capacity(2);

        if matches!(self.state, TimerState::Running) {
            self.state = TimerState::Stopped;
            events.push(TimerEvent::Ended(self.cycle.clone()));
            events.push(TimerEvent::Stopped);
            self.cycle = self.config.clone_first_cycle().unwrap();
            self.cycles_count = self.config.cycles_count.clone();
            self.started_at = None;
            self.elapsed = 0;
        }

        events
    }
}

impl Eq for Timer {}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state && self.cycle == other.cycle && self.elapsed() == other.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use mock_instant::{Instant, MockClock};

    use super::*;

    fn testing_timer() -> Timer {
        Timer {
            config: TimerConfig {
                cycles: TimerCycles::from([
                    TimerCycle::new("a", 3),
                    TimerCycle::new("b", 2),
                    TimerCycle::new("c", 1),
                ]),
                ..Default::default()
            },
            state: TimerState::Running,
            cycle: TimerCycle::new("a", 3),
            started_at: Some(Instant::now()),
            ..Default::default()
        }
    }

    #[test]
    fn running_infinite_timer() {
        let mut timer = testing_timer();

        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.cycle, TimerCycle::new("a", 3));

        // next ticks: state should still be running, cycle name
        // should be the same and cycle duration should be decremented
        // by 2

        MockClock::advance(Duration::from_secs(2));
        timer.update();

        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.cycle, TimerCycle::new("a", 1));

        // next tick: state should still be running, cycle should
        // switch to the next one

        MockClock::advance(Duration::from_secs(1));
        timer.update();

        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.cycle, TimerCycle::new("b", 2));

        // next ticks: state should still be running, cycle should
        // switch to the next one

        MockClock::advance(Duration::from_secs(2));
        timer.update();

        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.cycle, TimerCycle::new("c", 1));

        // next tick: state should still be running, cycle should
        // switch back to the first one

        MockClock::advance(Duration::from_secs(1));
        timer.update();

        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.cycle, TimerCycle::new("a", 3));
    }

    #[test]
    fn running_timer_events() {
        let mut timer = testing_timer();
        let mut events = Vec::new();

        // from a3 to b1
        MockClock::advance(Duration::from_secs(1));
        events.extend(timer.update());
        MockClock::advance(Duration::from_secs(1));
        events.extend(timer.update());
        MockClock::advance(Duration::from_secs(1));
        events.extend(timer.update());
        MockClock::advance(Duration::from_secs(1));
        events.extend(timer.update());

        assert_eq!(
            events,
            vec![
                TimerEvent::Running(TimerCycle::new("a", 3)),
                TimerEvent::Running(TimerCycle::new("a", 2)),
                TimerEvent::Running(TimerCycle::new("a", 1)),
                TimerEvent::Ended(TimerCycle::new("a", 0)),
                TimerEvent::Began(TimerCycle::new("b", 2)),
                TimerEvent::Running(TimerCycle::new("b", 2)),
            ]
        );
    }

    #[test]
    fn paused_timer_not_impacted_by_iterator() {
        let mut timer = testing_timer();
        timer.state = TimerState::Paused;
        let prev_timer = timer.clone();
        timer.update();
        assert_eq!(prev_timer, timer);
    }

    #[test]
    fn stopped_timer_not_impacted_by_iterator() {
        let mut timer = testing_timer();
        timer.state = TimerState::Stopped;
        let prev_timer = timer.clone();
        timer.update();
        assert_eq!(prev_timer, timer);
    }

    #[test]
    fn thread_safe_timer() {
        let mut timer = Timer::default();
        timer.config = testing_timer().config;
        timer.cycle = timer.config.clone_first_cycle().unwrap();
        timer.cycles_count = timer.config.cycles_count.clone();

        let mut events = Vec::new();

        assert_eq!(timer.state, TimerState::Stopped);
        assert_eq!(timer.cycle, TimerCycle::new("a", 3));

        events.extend(timer.start());
        events.extend(timer.set(21));

        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.cycle, TimerCycle::new("a", 21));

        events.extend(timer.pause());

        assert_eq!(timer.state, TimerState::Paused);
        assert_eq!(timer.cycle, TimerCycle::new("a", 21));

        events.extend(timer.resume());

        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.cycle, TimerCycle::new("a", 21));

        events.extend(timer.stop());

        assert_eq!(timer.state, TimerState::Stopped);
        assert_eq!(timer.cycle, TimerCycle::new("a", 3));

        assert_eq!(
            events,
            vec![
                TimerEvent::Started,
                TimerEvent::Began(TimerCycle::new("a", 3)),
                TimerEvent::Set(TimerCycle::new("a", 21)),
                TimerEvent::Paused(TimerCycle::new("a", 21)),
                TimerEvent::Resumed(TimerCycle::new("a", 21)),
                TimerEvent::Ended(TimerCycle::new("a", 21)),
                TimerEvent::Stopped,
            ]
        );
    }
}
