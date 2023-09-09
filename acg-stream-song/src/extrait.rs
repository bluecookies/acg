pub struct Sample {
    pub start_pos: SamplePosition,
    // TODO: 
    //  - length
    //  - wraparound
}

pub enum SamplePosition {
    Start,
    Random,  // TODO: random with and without wraparound
}

impl Sample {
    pub fn start() -> Self {
        Sample {
            start_pos: SamplePosition::Start,
        }
    }
}
