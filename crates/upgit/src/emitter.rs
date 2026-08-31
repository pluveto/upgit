use std::error::Error;

use upgit::Output;

pub struct Emitter {
    dest: Output,
}

impl Emitter {
    pub fn new(dest: Output) -> Self {
        Self { dest }
    }

    pub fn send(&self, urls: &[String]) -> Result<(), Box<dyn Error>> {
        let text = urls.join("\n");
        match self.dest {
            Output::Clipboard => {
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| format!("clipboard is unavailable: {e}"))?;
                clipboard
                    .set_text(&text)
                    .map_err(|e| format!("cannot copy URL to clipboard: {e}"))?;
            }
            Output::Stdout => println!("{text}"),
        }
        Ok(())
    }
}
