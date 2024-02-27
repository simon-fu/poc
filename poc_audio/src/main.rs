use rodio::source::{SineWave, Source};
use rodio::{OutputStream, Sink};
use std::time::Duration;

fn main() {
    test();
}

// refer https://files.seeedstudio.com/wiki/Grove-Speaker/res/Tone.pdf

pub const TONE_L6: f32= 440.0;
pub const TONE_L7: f32= 493.8833013;

pub const TONE_M1: f32= 523.2511306;
pub const TONE_M2: f32= 587.3295358;
pub const TONE_M3: f32= 659.2551138;
pub const TONE_M4: f32= 698.4564629;
pub const TONE_M5: f32= 783.990872;
pub const TONE_M6: f32= 880.0;
pub const TONE_M7: f32= 987.7666025;

pub const TONE_H1: f32= 1046.502261;


fn test() {
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    
    
    // https://www.qinyipu.com/jianpu/liuxinggequ/3118.html
    let chords = vec![
        ( TONE_M5, 1.0 ),
        ( TONE_M3, 0.5 ),
        ( TONE_M5, 0.5 ),
        ( TONE_H1, 1.5 ),
        ( TONE_M7, 0.5 ),
        ( TONE_M6, 1.0 ),
        ( TONE_H1, 1.0 ),
        ( TONE_M5, 2.0 ),

        ( TONE_M5, 1.0 ),
        ( TONE_M1, 0.5 ),
        ( TONE_M2, 0.5 ),
        ( TONE_M3, 1.0 ),
        ( TONE_M2, 0.5 ),
        ( TONE_M1, 0.5 ),
        ( TONE_M2, 3.0 ),

        ( TONE_M5, 1.0 ),
        ( TONE_M3, 0.5 ),
        ( TONE_M5, 0.5 ),
        ( TONE_H1, 1.5 ),
        ( TONE_M7, 0.5 ),
        ( TONE_M6, 1.0 ),
        ( TONE_H1, 1.0 ),
        ( TONE_M5, 2.0 ),

        ( TONE_M5, 1.0 ),
        ( TONE_M2, 0.5 ),
        ( TONE_M3, 0.5 ),
        ( TONE_M4, 1.5 ),
        ( TONE_L7, 0.5 ),
        ( TONE_M1, 3.0 ),

        ( TONE_M6, 1.0 ),
        ( TONE_H1, 1.0 ),
        ( TONE_H1, 2.0 ),

        ( TONE_M7, 1.0 ),
        ( TONE_M6, 0.5 ),
        ( TONE_M7, 0.5 ),
        ( TONE_H1, 2.0 ),

        ( TONE_M6, 0.5 ),
        ( TONE_M7, 0.5 ),
        ( TONE_H1, 0.5 ),
        ( TONE_M6, 0.5 ),
        ( TONE_M6, 0.5 ),
        ( TONE_M5, 0.5 ),
        ( TONE_M3, 0.5 ),
        ( TONE_M1, 0.5 ),
        ( TONE_M2, 3.0 ),

        ( TONE_M5, 1.0 ),
        ( TONE_M3, 0.5 ),
        ( TONE_M5, 0.5 ),
        ( TONE_H1, 1.5 ),
        ( TONE_M7, 0.5 ),
        ( TONE_M6, 1.0 ),
        ( TONE_H1, 1.0 ),
        ( TONE_M5, 2.0 ),

        ( TONE_M5, 1.0 ),
        ( TONE_M2, 0.5 ),
        ( TONE_M3, 0.5 ),
        ( TONE_M4, 1.5 ),
        ( TONE_L7, 0.5 ),
        ( TONE_M1, 3.0 ),
    ];

    for round in 1..999999 {
        println!("round {round}");
        let base_secs = 0.6;
        for item in chords.iter() {
            let duration = Duration::from_secs_f32(item.1 * base_secs);
            let source = SineWave::new(item.0)
            .take_duration( duration )
            .amplify(0.20);
            sink.append(source);
        }

        
        // The sound plays in a separate thread. This call will block the current thread until the sink
        // has finished playing all its queued sounds.
        sink.sleep_until_end();
    }

}
