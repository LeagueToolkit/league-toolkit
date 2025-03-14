//internal static float DecompressTime(ushort compressedTime, float duration) =>
//    compressedTime / ushort.MaxValue * duration;
//
//internal static ushort CompressTime(float time, float duration) => (ushort)(time / duration * ushort.MaxValue);

pub struct CompressedTime(pub u16);
impl CompressedTime {
    pub fn compress(time: f32, duration: f32) -> Self {
        CompressedTime(((time / duration) * u16::MAX as f32) as u16)
    }

    pub fn decompress(self, duration: f32) -> f32 {
        (self.0 as f32 / u16::MAX as f32) * duration
    }
}
