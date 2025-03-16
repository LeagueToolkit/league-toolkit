use glam::{Quat, Vec3};

pub struct TimedValue<T> {
    pub time: f32,
    pub value: T,
}

impl<T> TimedValue<T> {
    pub fn new(time: f32, value: T) -> Self {
        Self { time, value }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for TimedValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameValue")
            .field("time", &self.time)
            .field("value", &self.value)
            .finish()
    }
}

pub struct JointFrame {
    pub transform: TimedValue<Vec3>,
    pub rotation: TimedValue<Quat>,
    pub scale: TimedValue<Vec3>,
}

pub struct Frame {
    pub joint: u16,
    pub value: FrameValue,
}

pub enum FrameValue {
    Translation(TimedValue<Vec3>),
    Rotation(TimedValue<Quat>),
    Scale(TimedValue<Vec3>),
}
