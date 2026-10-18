//! État pur de l'assistant : aucun accès HID dans ces tests.
#[derive(Debug, Clone)]
pub struct Session {
    pub keys: usize,
    pub cols: usize,
    pub next: usize,
    pub presses: Vec<String>,
    pub mismatches: Vec<(usize, usize)>,
    pub image: String,
    pub brightness: String,
    pub orientation: Option<bool>,
    pub active: bool,
}
impl Session {
    pub fn new(keys: usize, cols: usize) -> Self {
        Self { keys, cols, next: 0, presses: vec![], mismatches: vec![], image: "En attente de confirmation".into(), brightness: "En attente".into(), orientation: None, active: true }
    }
    pub fn press(&mut self, raw: &str, index: usize) {
        if !self.active || self.next >= self.keys { return; }
        let expected = self.next;
        self.presses.push(format!("Appui {} : brut [{}] ; décodé {} ; attendu {} — {}", expected+1, raw, index+1, expected+1, if index==expected {"OK"} else {"ÉCHEC"}));
        if index != expected { self.mismatches.push((expected,index)); }
        self.next += 1;
    }
    pub fn reversed_rows(&self) -> bool {
        self.cols > 1 && self.next == self.keys && self.mismatches.len() >= self.keys / 2
            && self.mismatches.iter().all(|&(expected, got)| got == expected / self.cols * self.cols + self.cols - 1 - expected % self.cols)
    }
    pub fn result(&self) -> String {
        let mut text = format!("Touches : {}/{} appuis ; {} erreur(s)\n", self.next, self.keys, self.mismatches.len());
        if self.reversed_rows() { text.push_str("Diagnostic : ordre droite→gauche probable.\n"); }
        if self.orientation == Some(false) { text.push_str("Diagnostic : rotation de l'image incorrecte ?\n"); }
        for line in &self.presses { text.push_str(line); text.push('\n'); }
        text.push_str(&format!("Images : {}\nLuminosité : {}\n", self.image, self.brightness));
        text
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn mapping_and_orientation() {
        let mut s=Session::new(6,3);
        for i in [2,1,0,5,4,3] { s.press("01 AB",i); }
        assert!(s.reversed_rows());
        s.orientation=Some(false);
        assert!(s.result().contains("rotation de l'image incorrecte ?"));
        assert!(s.result().contains("brut [01 AB] ; décodé 3 ; attendu 1"));
        s.press("FF",0); assert_eq!(s.next,6);
    }
    #[test] fn incomplete_or_partial_not_declared_reversed() {
        let mut s=Session::new(6,3);
        s.press("01",2); assert!(!s.reversed_rows());
        for i in 1..6 { s.press("01",i); }
        assert!(!s.reversed_rows());
    }
}
