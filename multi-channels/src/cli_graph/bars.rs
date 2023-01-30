/// refer from https://github.com/juan-leon/lowcharts/blob/main/src/plot/matchbar.rs

use std::fmt;

use yansi::Color;

// use super::format::HorizontalScale;

use super::{Count, format::{align_left, bar_chars, align_right}};

pub fn display<'a>(rows: &'a [BarRow]) -> BarsDisplay<'a> {
    BarsDisplay::new(rows, 100)
}

pub fn display_with_width<'a>(rows: &'a [BarRow], width: usize) -> BarsDisplay<'a> {
    BarsDisplay::new(rows, width)
}

pub struct BarRow {
    pub label: String,
    pub count: Count,
}

pub struct BarsDisplay<'a> {
    rows: &'a [BarRow],
    max_count: Count,
    max_label_length: usize,
    width: usize
}

impl<'a> BarsDisplay<'a> {
    pub fn new(rows: &'a [BarRow], width: usize) -> Self {
        let mut max_label_length: usize = 0;
        let mut max_count: Count = 0;
        for row in rows.iter() {
            max_label_length = max_label_length.max(row.label.len());
            max_count = max_count.max(row.count);
        }
        Self {
            rows,
            max_count,
            max_label_length,
            width,
        }
    }
}

impl<'a> fmt::Display for BarsDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let term_width = f.width().unwrap_or(self.width);
        let count_width = format!("{}", self.max_count).len();
        let bar_width = term_width 
            - (1 + self.max_label_length + 1) 
            - 1 
            - (1 + count_width + 1)
            - 1
            - (1+1);

        // let horizontal_scale = HorizontalScale::new(self.max_count / bar_width as Count);
        // writeln!(f, "{horizontal_scale}")?;
        writeln!(f, "bar_width={}", bar_width)?;
        

        for row in self.rows.iter() {
            let bar_len = bar_width as Count * row.count / self.max_count;
            // let bar = Red.paint(format!("{:∎<width$}", "", width = bar_len as usize));
            let bar = Color::Yellow.paint(align_left(bar_chars(bar_len as usize), bar_width));
            // let count = Green.paint(format!("{units:width$}", units=row.count, width=count_width));
            let count = Color::Green.paint(align_right(row.count, count_width));
            writeln!(
                f,
                "[{label}] [{count}] [{bar}]",
                label = Color::Blue.paint(format!("{:width$}", row.label, width = self.max_label_length)),
                count = count, // horizontal_scale.get_count(row.count, count_width),
                bar = bar, // horizontal_scale.get_bar(row.count)
            )?;
        }

        // println!("plot args {:?}", (term_width, self.max_label_length, count_width, bar_width));

        Ok(())
    }
}


// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test() {
//         let rows = vec![
//             BarRow { label: "v1".into(), count: 8 },
//             BarRow { label: "v123".into(), count: 13 },
//         ];
//         println!("{}", display_with_width(&rows, 80));
//     }
// }

