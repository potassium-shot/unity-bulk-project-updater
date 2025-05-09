use std::path::PathBuf;

pub struct EguiPathBuf<T>(T);

impl Into<egui::WidgetText> for EguiPathBuf<&PathBuf> {
    fn into(self) -> egui::WidgetText {
        egui::WidgetText::from(self.0.to_string_lossy())
    }
}

impl egui::TextBuffer for EguiPathBuf<&PathBuf> {
    fn is_mutable(&self) -> bool {
        false
    }

    fn as_str(&self) -> &str {
        self.0
            .as_os_str()
            .to_str()
            .unwrap_or("<invalid utf-8 path>")
    }

    fn insert_text(&mut self, _: &str, _: usize) -> usize {
        unreachable!("is_mutable() is false");
    }

    fn delete_char_range(&mut self, _: std::ops::Range<usize>) {
        unreachable!("is_mutable() is false");
    }
}

impl egui::TextBuffer for EguiPathBuf<&mut PathBuf> {
    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        self.0
            .as_os_str()
            .to_str()
            .unwrap_or("<invalid utf-8 path>")
    }

    fn insert_text(&mut self, text: &str, char_index: usize) -> usize {
        match std::mem::take(self.0).into_os_string().into_string() {
            Ok(mut string) => {
                let inserted = string.insert_text(text, char_index);
                *self.0 = PathBuf::from(string);
                inserted
            }
            Err(orig) => {
                *self.0 = PathBuf::from(orig);
                0
            }
        }
    }

    fn delete_char_range(&mut self, char_range: std::ops::Range<usize>) {
        match std::mem::take(self.0).into_os_string().into_string() {
            Ok(mut string) => {
                string.delete_char_range(char_range);
                *self.0 = PathBuf::from(string);
            }
            Err(orig) => {
                *self.0 = PathBuf::from(orig);
            }
        }
    }
}

impl<T> EguiPathBuf<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }
}
