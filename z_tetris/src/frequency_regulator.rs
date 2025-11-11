pub struct FrequencyRegulator {
    events: usize,
    steps: usize,
    current_step: usize,
    events_generated: usize,
}

/// Calcullates how many events to generate each step to reach the desired frequency
/// I.e. if you want to generate 3 events every 4 steps, you would call
/// `FrequencyRegulator::new(3, 4)`. Then, each step, you would call `step()`
/// to get the number of events to generate this step.
impl FrequencyRegulator {
    pub fn new(events: usize, steps: usize) -> Self {
        FrequencyRegulator {
            events,
            steps,
            current_step: 0,
            events_generated: 0,
        }
    }

    pub fn get_events(&self) -> usize {
        self.events
    }

    pub fn get_steps(&self) -> usize {
        self.steps
    }

    pub fn set(&mut self, events: usize, steps: usize) {
        self.events = events;
        self.steps = steps;
        self.current_step = 0;
        self.events_generated = 0;
    }

    /// Returns the number of events to generate this step
    pub fn step(&mut self) -> usize {
        let events_to_generate_this_step =
            (self.events * (self.current_step + 1) + self.steps - 1) / self.steps - self.events_generated;

        self.events_generated += events_to_generate_this_step;

        self.current_step = (self.current_step + 1) % self.steps;

        if self.current_step == 0 {
            self.events_generated = 0;
        }

        events_to_generate_this_step
    }
}
