
use std::fmt::{self, Write};

pub fn bar_chars(n: usize) -> NChars {
    n_chars('∎', n)
}

pub fn n_chars(v: char, n: usize) -> NChars {
    NChars(v, n)
}

pub struct NChars(char, usize);

impl fmt::Display for NChars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        
        let (left, right) = match f.width() {
            Some(w) => {
                match f.align() {
                    Some(align) => {
                        let padding = w.max(self.1) - self.1;
                        match align {
                            fmt::Alignment::Left => (0, padding),
                            fmt::Alignment::Right => (padding, 0),
                            fmt::Alignment::Center => (padding/2, padding-padding/2),
                        }
                    },
                    None => (0, 0),
                }
            },
            None => (0, 0),
        };

        for _ in 0..left {
            f.write_char(f.fill())?;
        }
        
        // write!(f, "{:∎<width$}", "", width = self.0)
        for _ in 0..self.1 {
            f.write_char(self.0)?;
        }

        for _ in 0..right {
            f.write_char(f.fill())?;
        }

        Ok(())
    }
}

pub fn align_right<T>(v: T, w: usize) -> AlignRight<T> {
    AlignRight(v, w)
}

pub struct AlignRight<T>(T, usize);

impl<T> fmt::Display for AlignRight<T> 
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = f.width().unwrap_or(self.1);
        write!(f, "{val:>width$}", val=self.0, width=width)
    }
}

pub fn align_left<T>(v: T, w: usize) -> AlignSelf<T> {
    AlignSelf(v, w)
}

pub struct AlignSelf<T>(T, usize);

impl<T> fmt::Display for AlignSelf<T> 
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = f.width().unwrap_or(self.1);
        write!(f, "{val:<width$}", val=self.0, width=width)
    }
}

#[test] 
fn test() {
    assert_eq!(format!("[{}]", n_chars('1', 10)), "[1111111111]");
    assert_eq!(format!("[{}]", n_chars('1', 8)), "[11111111]");

    assert_eq!(format!("[{}]", align_right("12345", 10)), "[     12345]");
    assert_eq!(format!("[{}]", align_right("12345", 8)), "[   12345]");

    assert_eq!(format!("[{}]", align_left("12345", 10)), "[12345     ]");
    assert_eq!(format!("[{}]", align_left("12345", 8)), "[12345   ]");

    assert_eq!(format!("[{}]", align_left(n_chars('1', 3), 10)),  "[111       ]") ;
    assert_eq!(format!("[{}]", align_right(n_chars('1', 3), 10)), "[       111]") ;
}

